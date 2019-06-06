extern crate tobj;

use std::path::Path;
use std::hash::{Hash, Hasher};
use std::time::Instant;
use std::collections::HashMap;
use gfx_hal::pso;
use gfx_hal::format as f;

use glm::{Mat4,Vec2,Vec3,vec3,vec2};
use super::utils::hash_float;

use log::debug;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub pos: Vec3,
    pub color: Vec3,
    pub tex_coord: Vec2
}

impl Hash for Vertex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (0..3).for_each(|i| hash_float(self.pos[i], state));
        (0..3).for_each(|i| hash_float(self.color[i], state));
        (0..2).for_each(|i| hash_float(self.tex_coord[i], state));
    }
}

impl Eq for Vertex {}

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
                format: f::Format::Rgb32Sfloat,
                offset: 0,
            },
        },
        pso::AttributeDesc {
            location: 1,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rgb32Sfloat,
                offset: std::mem::size_of::<Vec3>() as _
            },
        },
        pso::AttributeDesc {
            location: 2,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: (std::mem::size_of::<Vec3>() * 2) as _
            },
        }
    ];
}

macro_rules! vert {
    ( $x:expr, $y:expr, $z: expr, $r:expr, $g:expr, $b:expr, $tx:expr, $ty:expr ) => {
        Vertex {
            pos: vec3($x, $y, $z),
            color: vec3($r, $g, $b),
            tex_coord: vec2($tx, $ty)
        }
    };
}

#[derive(Debug, Clone, Copy)]
pub struct UniformBufferObject {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4
}

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indicies: Vec<u32>
}

impl Model {
    pub fn load(file: &Path) -> Self {
        debug!("Reading model file: {:?}", file);
        let start_time = Instant::now();
        let (models, _materials) = tobj::load_obj(file).unwrap();
        debug!("Reading took {}s", start_time.elapsed().as_millis() as f64 / 1000.0);
        debug!("Processing {} models", models.len());
        let mut vertices = Vec::new();
        let mut indicies = Vec::new();
        let mut unique_vertices: HashMap<Vertex, u32> = HashMap::new();

        for model in models {
            let mesh = &model.mesh;
            debug!("Processing {} indicies", &mesh.indices.len());
            for index in &mesh.indices {
                let i = *index as usize;
                let vertex = vert!(
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                    1.0, 1.0, 1.0,
                    mesh.texcoords[i * 2],
                    1.0 - mesh.texcoords[i * 2 + 1]
                );
                let index = match unique_vertices.get(&vertex) {
                    Some(&idx) => idx,
                    None => {
                        vertices.push(vertex);
                        let idx = (vertices.len() - 1) as u32;
                        unique_vertices.insert(vertex, idx);
                        idx
                    }
                };
                indicies.push(index);
            }
        }
        debug!("Done loading model: {:?}. Took: {}s", file, start_time.elapsed().as_millis() as f64 / 1000.0);
        Model {
            vertices,
            indicies
        }
    }
}
