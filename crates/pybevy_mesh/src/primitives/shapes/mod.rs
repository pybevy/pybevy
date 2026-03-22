pub mod dim2;
pub mod dim3;

// Re-export 2D shapes
pub use dim2::{
    PyAnnulus, PyCapsule2d, PyCircle, PyCircularSector, PyCircularSegment, PyEllipse, PyRectangle,
    PyRegularPolygon, PyRhombus, PySegment2d, PyTriangle2d,
};
// Re-export 3D shapes
pub use dim3::{
    PyCapsule3d, PyCone, PyCuboid, PyCylinder, PyPlane3d, PySphere, PyTetrahedron, PyTorus,
    PyTriangle3d,
};
