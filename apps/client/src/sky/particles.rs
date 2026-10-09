//! The exact soft-disc fragment from game/effects.go, with Bevy's mesh vertex path.
use bevy::{
    asset::embedded_asset,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey, MaterialPlugin},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct ParticleMaterial {
    // A portable, aligned material binding; tint is per vertex, not per draw.
    #[uniform(0)]
    pub unused: Vec4,
}
impl Material for ParticleMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://earth_two_client/sky/particles.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}
pub struct ParticleMaterialPlugin;
impl Plugin for ParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "particles.wesl");
        app.add_plugins(MaterialPlugin::<ParticleMaterial>::default());
    }
}
