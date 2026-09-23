use bevy::prelude::*;
use mujoco_rs::wrappers::{MjSpec, MjtObj};

macro_rules! get_model_vec {
    ($model:ident, $id:expr, $name:ident) => {
        paste::paste! {{
            let start = $model.[<$name adr>]()[$id] as usize;
            let count = start + $model.[<$name num>]()[$id] as usize;
            &$model.$name()[start..count]
        }}
    };
}

#[test]
fn play_user_mesh() -> Result {
    let mut spec = MjSpec::from_xml_string(
        r#"
<mujoco>
    <asset>
        <mesh name="user_mesh" vertex="0 0 0  1 0 0  1 1 0  0 1 1" />
    </asset>
</mujoco>
"#,
    )?;

    spec.mesh_mut("user_mesh")
        .unwrap()
        .set_uservert(&[0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 1.]);

    let model = spec.compile()?;

    let id = model
        .name_to_id(MjtObj::mjOBJ_MESH, "user_mesh")
        .expect("mesh exists");

    println!("Mesh vertices: {:#?}", get_model_vec!(model, id, mesh_vert));
    println!("Mesh faces: {:#?}", get_model_vec!(model, id, mesh_face));
    println!(
        "Mesh normals: {:#?}",
        get_model_vec!(model, id, mesh_normal)
    );

    println!("Mesh face count: {:?}", model.mesh_face());
    println!("Mesh face normal count: {:?}", model.mesh_facenormal());
    println!("Mesh face texcoord count: {:?}", model.mesh_facetexcoord());

    println!("Mesh vertex count: {:#?}", model.mesh_vert().len());
    println!("Mesh normal count: {:#?}", model.mesh_normal().len());
    println!("Mesh texcoord count: {:#?}", model.mesh_texcoord().len());

    Ok(())
}

mod assumptions {
    use super::*;

    #[test]
    fn test_generates_convex_hull() -> Result {
        let mut spec = MjSpec::from_xml_string(
            r#"
<mujoco>
    <asset>
        <mesh name="user_mesh" vertex="0 0 0  1 0 0  1 1 0  0 1 1" />
    </asset>
</mujoco>
"#,
        )?;

        let model = spec.compile()?;

        let id = model
            .name_to_id(MjtObj::mjOBJ_MESH, "user_mesh")
            .expect("mesh exists");

        let vertices = get_model_vec!(model, id, mesh_vert);
        let faces = get_model_vec!(model, id, mesh_face);

        assert_eq!(vertices.len(), 4);
        assert_eq!(faces.len(), 4);

        Ok(())
    }

    #[test]
    fn test_compile_fails_with_invalid_geometry() -> Result {
        let mut spec = MjSpec::from_xml_string(
            r#"
<mujoco>
    <asset>
        <mesh name="user_mesh" />
    </asset>
</mujoco>"#,
        )?;

        assert!(spec.compile().is_err());

        spec.mesh_mut("user_mesh").unwrap().set_uservert(
            [
                [f32::INFINITY, 0., 0.],
                [1., 0., 0.],
                [1., 1., 0.],
                [0., 1., 1.],
            ]
            .as_flattened(),
        );

        assert!(spec.compile().is_err());

        spec.mesh_mut("user_mesh").unwrap().set_uservert(
            [[f32::NAN, 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 1.]].as_flattened(),
        );

        assert!(spec.compile().is_err());

        spec.mesh_mut("user_mesh")
            .unwrap()
            .set_uservert(&[0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 1.]);

        assert!(spec.compile().is_ok());

        spec.mesh_mut("user_mesh")
            .unwrap()
            .set_userface(&[1, 2, 3, 1, 3 /* missing */]);

        println!("Compile result: {:#?}", spec.compile());

        assert!(spec.compile().is_err());

        spec.mesh_mut("user_mesh")
            .unwrap()
            .set_userface(&[1, 2, 3, 1, 3, 1]);

        println!("Compile result: {:#?}", spec.compile());

        assert!(spec.compile().is_err());

        Ok(())
    }

    #[test]
    fn test_indices_always_same_length() -> Result {
        let mut spec = MjSpec::from_xml_string(
            r#"
<mujoco>
    <asset>
        <mesh name="user_mesh" vertex="0 0 0  1 0 0  1 1 0  0 1 1" face="0 1 2  0 2 3" />
    </asset>
</mujoco>"#,
        )?;

        let model = spec.compile()?;

        let face_count = model.mesh_face().len();
        let normal_count = model.mesh_facenormal().len();
        let texcoord_count = model.mesh_facetexcoord().len();

        assert_eq!(face_count, normal_count);
        assert_eq!(face_count, texcoord_count);

        spec.mesh_mut("user_mesh").unwrap().set_userface(&[0, 1, 2]);

        assert!(spec.compile().is_err());

        spec.mesh_mut("user_mesh")
            .unwrap()
            .set_userface(&[0, 1, 2, 0, 2, 3]);

        assert!(spec.compile().is_ok());

        spec.mesh_mut("user_mesh")
            .unwrap()
            .set_usernormal(&[0., 0., 1., 0., 0., 1., 0., 0., 1. /* missing */]);

        let model = spec.compile()?;

        println!("Mesh normals: {:?}", model.mesh_normal());
        println!("Mesh face normals: {:?}", model.mesh_facenormal());

        Ok(())
    }
}
