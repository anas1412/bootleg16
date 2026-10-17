//! A sequence number from another model (a weapon switch) must never panic.
use goldsrc::mdl::Model;

#[test]
fn a_sequence_number_past_the_end_is_clamped() {
    let path = format!("{}/../dist/gamedata/models/v_knife.mdl", env!("CARGO_MANIFEST_DIR"));
    let Ok(model) = Model::load(&path) else { return }; // no game data here: nothing to check
    let past = model.sequences.len() + 5;
    let last = model.pose_at(model.sequences.len() - 1, 0.0, 0.0);
    assert_eq!(model.pose_at(past, 0.0, 0.0).len(), last.len());
}

#[test]
fn a_player_model_without_textures_takes_them_from_its_t_file() {
    let path = format!("{}/../dist/gamedata/models/player/urban/urban.mdl", env!("CARGO_MANIFEST_DIR"));
    let Ok(model) = Model::load(&path) else { return };
    assert!(!model.textures().is_empty(), "urban.mdl has no textures of its own: they are in urbanT.mdl");
}
