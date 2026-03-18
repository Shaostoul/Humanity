//! CSG boolean operations — union, subtract, intersect on meshes.

/// CSG operation type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CsgOp {
    Union,
    Subtract,
    Intersect,
}

/// A CSG brush primitive.
pub struct CsgBrush {
    pub op: CsgOp,
    // TODO: vertex data, transform
}

impl CsgBrush {
    pub fn new(op: CsgOp) -> Self {
        Self { op }
    }
}
