//! Exhaustive and pseudo-random invariant tests for the layout core, using
//! only std. For the small shapes here, exhaustive enumeration of every basic
//! slice is stronger than random sampling. The key safety invariant: no planned
//! slice or broadcast stride ever yields an offset outside its storage.

use pybevy_array::{
    ArrayError, ArrayStorage, DenseArrayCore, IndexOp, Layout, Scalar, broadcast_shapes,
    broadcast_strides,
};

fn contiguous_f64(shape: &[usize]) -> DenseArrayCore {
    let n: usize = shape.iter().product();
    let data: Vec<f64> = (0..n).map(|i| i as f64).collect();
    DenseArrayCore::from_storage(ArrayStorage::Float64(data), shape).unwrap()
}

/// A plain-loop reference for `range(n)[start:stop:step]`, independent of the
/// crate's `resolve_slice`, to differentially check planned slices.
fn reference_slice(start: isize, stop: isize, step: isize, n: isize) -> Vec<isize> {
    assert!(step != 0);
    let wrap = |v: isize| if v < 0 { v + n } else { v };
    let (lo, hi) = if step < 0 { (-1, n - 1) } else { (0, n) };
    let clamp = |v: isize| v.max(lo).min(hi);
    let start = clamp(wrap(start));
    let stop = clamp(wrap(stop));
    let mut out = Vec::new();
    let mut i = start;
    if step > 0 {
        while i < stop {
            if (0..n).contains(&i) {
                out.push(i);
            }
            i += step;
        }
    } else {
        while i > stop {
            if (0..n).contains(&i) {
                out.push(i);
            }
            i += step;
        }
    }
    out
}

#[test]
fn all_1d_basic_slices_match_reference_and_stay_in_bounds() {
    for n in 0usize..=6 {
        let array = contiguous_f64(&[n]);
        let ni = n as isize;
        for step in [-2isize, -1, 1, 2] {
            for start in -(ni + 1)..=(ni + 1) {
                for stop in -(ni + 1)..=(ni + 1) {
                    let op = IndexOp::Slice {
                        start: Some(start),
                        stop: Some(stop),
                        step,
                    };
                    let plan = array.plan(&[op]).unwrap();
                    let offsets: Vec<usize> = plan.iter_offsets().collect();
                    for &o in &offsets {
                        assert!(o < n.max(1), "offset {o} out of bounds for n={n}");
                    }
                    let expected = reference_slice(start, stop, step, ni);
                    let got: Vec<isize> = offsets.iter().map(|&o| o as isize).collect();
                    assert_eq!(got, expected, "n={n} {start}:{stop}:{step}");
                }
            }
        }
    }
}

#[test]
fn all_2d_integer_and_column_indices_stay_in_bounds() {
    for rows in 1usize..=4 {
        for cols in 1usize..=4 {
            let array = contiguous_f64(&[rows, cols]);
            let len = rows * cols;
            // Every row, every column, every element.
            for r in 0..rows {
                let row = array.plan(&[IndexOp::Index(r as isize)]).unwrap();
                assert_eq!(row.shape, vec![cols]);
                for o in row.iter_offsets() {
                    assert!(o < len);
                }
            }
            for c in 0..cols {
                let col = array
                    .plan(&[IndexOp::full(), IndexOp::Index(c as isize)])
                    .unwrap();
                assert_eq!(col.shape, vec![rows]);
                let offsets: Vec<usize> = col.iter_offsets().collect();
                let expected: Vec<usize> = (0..rows).map(|r| r * cols + c).collect();
                assert_eq!(offsets, expected);
            }
        }
    }
}

#[test]
fn negative_indices_match_positive_counterparts() {
    let array = contiguous_f64(&[5]);
    for i in 0..5isize {
        let pos = array.plan(&[IndexOp::Index(i)]).unwrap();
        let neg = array.plan(&[IndexOp::Index(i - 5)]).unwrap();
        assert_eq!(
            pos.iter_offsets().collect::<Vec<_>>(),
            neg.iter_offsets().collect::<Vec<_>>()
        );
    }
}

#[test]
fn negative_resulting_offset_errors_instead_of_clamping() {
    let malformed = Layout {
        shape: vec![2],
        strides: vec![-1],
        offset: 0,
    };

    assert!(matches!(
        malformed.index(&[IndexOp::Index(1)]),
        Err(ArrayError::Overflow("layout offset"))
    ));
}

#[test]
fn slice_copy_reads_exactly_the_planned_offsets() {
    let shapes: [&[usize]; 4] = [&[6], &[2, 3], &[3, 2], &[2, 2, 2]];
    for shape in shapes {
        let array = contiguous_f64(shape);
        // A strided slice on the first axis plus a full remainder.
        let mut ops = vec![IndexOp::Slice {
            start: Some(0),
            stop: None,
            step: 2,
        }];
        ops.extend((1..shape.len()).map(|_| IndexOp::full()));
        let plan = array.plan(&ops).unwrap();
        let expected: Vec<Scalar> = plan
            .iter_offsets()
            .map(|offset| Scalar::F64(offset as f64))
            .collect();
        let copy = array.slice_copy(&ops).unwrap();
        assert_eq!(copy.to_scalars().unwrap(), expected);
    }
}

#[test]
fn reshape_roundtrip_preserves_elements_for_all_factorings() {
    // 24 elements reshaped through every 2- and 3-factor shape.
    let base = contiguous_f64(&[24]);
    let shapes: [&[usize]; 6] = [&[24], &[2, 12], &[12, 2], &[4, 6], &[2, 3, 4], &[6, 2, 2]];
    for shape in shapes {
        let reshaped = base.reshape(shape).unwrap();
        assert_eq!(reshaped.shape(), shape);
        assert_eq!(
            reshaped.ravel().unwrap().to_scalars().unwrap(),
            base.to_scalars().unwrap()
        );
    }
}

#[test]
fn broadcast_strides_stay_in_source_bounds_exhaustive() {
    let candidates: [&[usize]; 8] = [
        &[],
        &[1],
        &[3],
        &[1, 3],
        &[2, 1],
        &[2, 3],
        &[1, 1, 3],
        &[2, 3, 4],
    ];
    for a in candidates {
        for b in candidates {
            let Ok(result) = broadcast_shapes(a, b) else {
                continue;
            };
            // Symmetry of validity.
            assert!(broadcast_shapes(b, a).is_ok());
            for shape in [a, b] {
                let src_len: usize = shape.iter().product();
                let layout = Layout::c_contiguous(shape).unwrap();
                let strides = broadcast_strides(&layout, &result).unwrap();
                let view = Layout {
                    shape: result.clone(),
                    strides,
                    offset: 0,
                };
                for off in view.iter_offsets() {
                    assert!(off < src_len.max(1), "broadcast offset {off} >= {src_len}");
                }
            }
        }
    }
}

/// A tiny LCG so we can push wider pseudo-random coverage without a dev-dep.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 16
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next() as usize) % (hi - lo + 1)
    }
}

#[test]
fn random_3d_slices_never_escape_storage() {
    let mut rng = Lcg(0x9E3779B97F4A7C15);
    for _ in 0..5000 {
        let shape = [rng.range(1, 5), rng.range(1, 5), rng.range(1, 5)];
        let array = contiguous_f64(&shape);
        let len: usize = shape.iter().product();
        let steps = [-2isize, -1, 1, 2];
        let ops: Vec<IndexOp> = shape
            .iter()
            .map(|&size| {
                if rng.next().is_multiple_of(2) {
                    IndexOp::Index(rng.range(0, size - 1) as isize)
                } else {
                    let span = size as isize + 1;
                    IndexOp::Slice {
                        start: Some(rng.range(0, 2 * size) as isize - size as isize),
                        stop: Some((rng.next() as isize % (2 * span)) - span),
                        step: steps[rng.range(0, 3)],
                    }
                }
            })
            .collect();
        let plan = array.plan(&ops).unwrap();
        for off in plan.iter_offsets() {
            assert!(off < len, "offset {off} >= len {len} for shape {shape:?}");
        }
    }
}
