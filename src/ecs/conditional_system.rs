use pybevy_ecs::shared::schedule::ConditionExpr;
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyAttributeError, PyValueError},
    prelude::*,
};

const CONDITION_EXPR_ATTR: &str = "__pybevy_condition_expr__";
const MAX_CONDITION_EXPR_DEPTH: usize = 64;

pub(crate) fn extract_condition_expr(condition: Py<PyAny>) -> PyResult<ConditionExpr<Py<PyAny>>> {
    extract_condition_expr_at(condition, 0)
}

fn extract_condition_expr_at(
    condition: Py<PyAny>,
    depth: usize,
) -> PyResult<ConditionExpr<Py<PyAny>>> {
    if depth > MAX_CONDITION_EXPR_DEPTH {
        return Err(PyValueError::new_err(
            "condition expression nesting exceeds 64 levels",
        ));
    }

    Python::attach(|py| {
        let marker = match condition.bind(py).getattr(CONDITION_EXPR_ATTR) {
            Ok(marker) => marker,
            Err(error) if error.is_instance_of::<PyAttributeError>(py) => {
                return Ok(ConditionExpr::Leaf(condition));
            }
            Err(error) => return Err(error),
        };
        let (operator, operands) = marker.extract::<(String, Vec<Py<PyAny>>)>()?;
        let mut operands = operands.into_iter();

        match operator.as_str() {
            "and" | "or" => {
                let Some(first) = operands.next() else {
                    return Ok(ConditionExpr::Leaf(condition));
                };
                let mut expression = extract_condition_expr_at(first, depth + 1)?;
                for operand in operands {
                    let right = extract_condition_expr_at(operand, depth + 1)?;
                    expression = if operator == "and" {
                        ConditionExpr::And(Box::new(expression), Box::new(right))
                    } else {
                        ConditionExpr::Or(Box::new(expression), Box::new(right))
                    };
                }
                Ok(expression)
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
                Ok(ConditionExpr::Not(Box::new(extract_condition_expr_at(
                    operand,
                    depth + 1,
                )?)))
            }
            _ => Err(PyValueError::new_err(
                "unknown condition expression operator",
            )),
        }
    })
}

/// Wrapper for a system with a run condition
/// Similar to Bevy's IntoSystemConfigs::run_if()
#[pyclass(name = "ConditionalSystem", from_py_object)]
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
    pub fn new(system: Py<PyAny>, condition: Py<PyAny>) -> PyResult<Self> {
        Ok(Self {
            system,
            condition: extract_condition_expr(condition)?,
        })
    }

    /// Proxy to the inner system's __name__ for introspection paths.
    #[getter]
    pub fn __name__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.system.bind(py).getattr("__name__")?.unbind())
    }

    /// Combine this condition with another using AND logic
    /// Both conditions must return true for the system to run
    /// Usage: run_if(system, condition1).and_(condition2)
    pub fn and_(&self, condition: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| {
            Ok(Self {
                system: self.system.clone_ref(py),
                condition: ConditionExpr::And(
                    Box::new(
                        self.condition
                            .map_ref(&mut |condition| condition.clone_ref(py)),
                    ),
                    Box::new(extract_condition_expr(condition)?),
                ),
            })
        })
    }

    /// Combine this condition with another using OR logic
    /// Either condition must return true for the system to run
    /// Usage: run_if(system, condition1).or_(condition2)
    pub fn or_(&self, condition: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| {
            Ok(Self {
                system: self.system.clone_ref(py),
                condition: ConditionExpr::Or(
                    Box::new(
                        self.condition
                            .map_ref(&mut |condition| condition.clone_ref(py)),
                    ),
                    Box::new(extract_condition_expr(condition)?),
                ),
            })
        })
    }

    /// Negate this condition using NOT logic
    /// The condition must return false for the system to run
    /// Usage: run_if(system, condition).not_()
    pub fn not_(&self) -> PyResult<Self> {
        Python::attach(|py| {
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

/// Helper function to create a conditional system
/// Usage: run_if(system_func, condition_func)
#[pyfunction]
pub fn run_if(system: Py<PyAny>, condition: Py<PyAny>) -> PyResult<PyConditionalSystem> {
    PyConditionalSystem::new(system, condition)
}
