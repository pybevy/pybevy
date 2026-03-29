pub mod dim2;
pub mod dim3;

pub use dim2::{
    PyAnnulus, PyCapsule2d, PyCircle, PyCircularSector, PyCircularSegment, PyEllipse, PyRectangle,
    PyRegularPolygon, PyRhombus, PySegment2d, PyTriangle2d,
};
pub use dim3::{
    PyCapsule3d, PyCone, PyCuboid, PyCylinder, PyPlane3d, PySphere, PyTetrahedron, PyTorus,
    PyTriangle3d,
};
