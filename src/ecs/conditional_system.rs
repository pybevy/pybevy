use pybevy_ecs::shared::schedule::ConditionExpr;
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyAttributeError, PyTypeError, PyValueError},
    prelude::*,
};

use crate::ecs::decorated_callable::{CallableKind, strip_decorated, unwrap_decorated};

const CONDITION_EXPR_ATTR: &str = "__pybevy_condition_expr__";
const MAX_CONDITION_EXPR_DEPTH: usize = 64;

pub(crate) fn extract_condition_expr(condition: Py<PyAny>) -> PyResult<ConditionExpr<Py<PyAny>>> {
    Ok(extract_condition_expr_at(condition, 0)?.0)
}

/// Extracts one marker, returning the expression and its generated tree depth.
///
/// The marker-depth check alone is not enough: folding a flat operand list
/// builds a left-deep tree far deeper than any input marker nesting, and
/// recursive lowering or teardown of that tree exhausts the stack. Tracking
/// the generated depth during the fold keeps the constructed tree bounded.
fn extract_condition_expr_at(
    condition: Py<PyAny>,
    depth: usize,
) -> PyResult<(ConditionExpr<Py<PyAny>>, usize)> {
    if depth > MAX_CONDITION_EXPR_DEPTH {
        return Err(PyValueError::new_err(
            "condition expression nesting exceeds 64 levels",
        ));
    }

    Python::attach(|py| {
        let condition = strip_decorated(condition.bind(py), CallableKind::Condition)?.unbind();
        let marker = match condition.bind(py).getattr(CONDITION_EXPR_ATTR) {
            Ok(marker) => marker,
            Err(error) if error.is_instance_of::<PyAttributeError>(py) => {
                return Ok((ConditionExpr::Leaf(condition), 0));
            }
            Err(error) => return Err(error),
        };
        let (operator, operands) = marker.extract::<(String, Vec<Py<PyAny>>)>()?;
        let mut operands = operands.into_iter();

        match operator.as_str() {
            "and" | "or" => {
                let Some(first) = operands.next() else {
                    return Ok((ConditionExpr::Leaf(condition), 0));
                };
                let (mut expression, mut expression_depth) =
                    extract_condition_expr_at(first, depth + 1)?;
                for operand in operands {
                    let (right, right_depth) = extract_condition_expr_at(operand, depth + 1)?;
                    let folded_depth = expression_depth.max(right_depth) + 1;
                    if folded_depth > MAX_CONDITION_EXPR_DEPTH {
                        return Err(PyValueError::new_err(
                            "condition expression nesting exceeds 64 levels",
                        ));
                    }
                    expression_depth = folded_depth;
                    expression = if operator == "and" {
                        ConditionExpr::And(Box::new(expression), Box::new(right))
                    } else {
                        ConditionExpr::Or(Box::new(expression), Box::new(right))
                    };
                }
                Ok((expression, expression_depth))
            }
            "not" => {
                let operand = operands.next().ok_or_else(|| {
                    PyValueError::new_err("not condition expression requires one operand")
                })?;
                if operands.next().is_some() {
                    return Err(PyValueError::new_err(
                        "not condition expression requires one operand",
                    ));
                }
                let (inner, inner_depth) = extract_condition_expr_at(operand, depth + 1)?;
                if inner_depth + 1 > MAX_CONDITION_EXPR_DEPTH {
                    return Err(PyValueError::new_err(
                        "condition expression nesting exceeds 64 levels",
                    ));
                }
                Ok((ConditionExpr::Not(Box::new(inner)), inner_depth + 1))
            }
            _ => Err(PyValueError::new_err(
                "unknown condition expression operator",
            )),
        }
    })
}

/// Wrapper for a system with a run condition
/// Similar to Bevy's IntoSystemConfigs::run_if()
#[pyclass(name = "ConditionalSystem", module = "pybevy.ecs", from_py_object)]
pub struct PyConditionalSystem {
    /// The system function to run
    pub system: Py<PyAny>,
    pub(crate) condition: ConditionExpr<Py<PyAny>>,
}

impl Clone for PyConditionalSystem {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            system: self.system.clone_ref(py),
            condition: self
                .condition
                .map_ref(&mut |condition| condition.clone_ref(py)),
        })
    }
}

#[pymethods]
impl PyConditionalSystem {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.system)?;
        let mut result = Ok(());
        self.condition.for_each_leaf(&mut |leaf| {
            if result.is_ok() {
                result = visit.call(leaf);
            }
        });
        result
    }

    #[new]
    pub fn py_new(system: &Bound<'_, PyAny>, condition: &Bound<'_, PyAny>) -> PyResult<Self> {
        if system.is_instance_of::<PyConditionalSystem>() {
            return Err(PyTypeError::new_err(
                "run_if() cannot wrap a ConditionalSystem; use .and_(condition)",
            ));
        }
        let system = unwrap_decorated(system, CallableKind::System, "run_if()")?;
        Self::new(system.unbind(), condition_callable(condition, "run_if()")?)
    }

    /// Proxy to the inner system's __name__ for introspection paths.
    #[getter]
    pub fn __name__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.system.bind(py).getattr("__name__")?.unbind())
    }

    /// Combine this condition with another using AND logic
    /// Both conditions must return true for the system to run
    /// Usage: run_if(system, condition1).and_(condition2)
    pub fn and_(&self, condition: &Bound<'_, PyAny>) -> PyResult<Self> {
        let condition = condition_callable(condition, "and_()")?;
        Python::attach(|py| {
            let operand = extract_condition_expr_at(condition, 0)?;
            ensure_condition_depth(self.condition.depth().max(operand.1) + 1)?;
            Ok(Self {
                system: self.system.clone_ref(py),
                condition: ConditionExpr::And(
                    Box::new(
                        self.condition
                            .map_ref(&mut |condition| condition.clone_ref(py)),
                    ),
                    Box::new(operand.0),
                ),
            })
        })
    }

    /// Combine this condition with another using OR logic
    /// Either condition must return true for the system to run
    /// Usage: run_if(system, condition1).or_(condition2)
    pub fn or_(&self, condition: &Bound<'_, PyAny>) -> PyResult<Self> {
        let condition = condition_callable(condition, "or_()")?;
        Python::attach(|py| {
            let operand = extract_condition_expr_at(condition, 0)?;
            ensure_condition_depth(self.condition.depth().max(operand.1) + 1)?;
            Ok(Self {
                system: self.system.clone_ref(py),
                condition: ConditionExpr::Or(
                    Box::new(
                        self.condition
                            .map_ref(&mut |condition| condition.clone_ref(py)),
                    ),
                    Box::new(operand.0),
                ),
            })
        })
    }

    /// Negate this condition using NOT logic
    /// The condition must return false for the system to run
    /// Usage: run_if(system, condition).not_()
    pub fn not_(&self) -> PyResult<Self> {
        Python::attach(|py| {
            ensure_condition_depth(self.condition.depth() + 1)?;
            Ok(Self {
                system: self.system.clone_ref(py),
                condition: ConditionExpr::Not(Box::new(
                    self.condition
                        .map_ref(&mut |condition| condition.clone_ref(py)),
                )),
            })
        })
    }
}

/// Rejects expression trees deeper than the documented cap.
///
/// Constructed expressions are bounded by this check, so later recursive
/// lowering, traversal, and teardown cannot exhaust the stack.
fn ensure_condition_depth(depth: usize) -> PyResult<()> {
    if depth > MAX_CONDITION_EXPR_DEPTH {
        return Err(PyValueError::new_err(
            "condition expression nesting exceeds 64 levels",
        ));
    }
    Ok(())
}

impl PyConditionalSystem {
    /// A conditional system over an already unwrapped system and condition.
    pub(crate) fn new(system: Py<PyAny>, condition: Py<PyAny>) -> PyResult<Self> {
        Ok(Self {
            system,
            condition: extract_condition_expr(condition)?,
        })
    }
}

fn condition_callable(condition: &Bound<'_, PyAny>, entry: &str) -> PyResult<Py<PyAny>> {
    Ok(unwrap_decorated(condition, CallableKind::Condition, entry)?.unbind())
}

/// Helper function to create a conditional system
/// Usage: run_if(system_func, condition_func)
#[pyfunction]
pub fn run_if(
    system: &Bound<'_, PyAny>,
    condition: &Bound<'_, PyAny>,
) -> PyResult<PyConditionalSystem> {
    PyConditionalSystem::py_new(system, condition)
}
