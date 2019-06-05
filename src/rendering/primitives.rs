use gfx_hal::pso;
use gfx_hal::format as f;

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 3],
    pub tex_coord: [f32; 2]
}

impl Vertex {

    pub const BINDING_DESCRIPTION: pso::VertexBufferDesc = pso::VertexBufferDesc {
        binding: 0,
        stride: std::mem::size_of::<Vertex>() as u32,
        rate: pso::VertexInputRate::Vertex,
    };

    pub const ATTRIBUTE_DESCRIPTIONS: [pso::AttributeDesc; 3] = [
        pso::AttributeDesc {
            location: 0,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 0,
            },
        },
        pso::AttributeDesc {
            location: 1,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rgb32Sfloat,
                offset: std::mem::size_of::<[f32; 2]>() as _
            },
        },
        pso::AttributeDesc {
            location: 2,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: std::mem::size_of::<[f32; 5]>() as _
            },
        }
    ];
}

macro_rules! vert {
    ( $x:expr, $y:expr, $r:expr, $g:expr, $b:expr, $tx:expr, $ty:expr ) => {
        Vertex {
            pos: [$x, $y],
            color: [$r, $g, $b],
            tex_coord: [$tx, $ty]
        }
    };
}

pub(super) const VERTICIES: [Vertex; 4] = [
    vert!(-0.5, -0.5, 1.0, 0.0, 0.0, 1.0, 0.0),
    vert!( 0.5, -0.5, 0.0, 1.0, 0.0, 0.0, 0.0),
    vert!( 0.5,  0.5, 0.0, 0.0, 1.0, 0.0, 1.0),
    vert!(-0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0)
];

pub(super) const INDICIES: [u16; 6] = [
    0, 1, 2, 2, 3, 0
];

//#[derive(Debug, Clone, Copy)]
//pub struct Mat4([[f32; 4]; 4]);
use glm::Mat4;

#[derive(Debug, Clone, Copy)]
pub struct UniformBufferObject {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4
}

impl UniformBufferObject {
    pub fn center() -> Self {
        UniformBufferObject {
            model: Mat4::identity(),
            view: Mat4::identity(),
            proj: Mat4::identity()
        }
    }
}
