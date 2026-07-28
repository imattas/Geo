use geo_bootstrap::{bootstrap_stages, StageKind};

#[test]
fn bootstrap_stages_start_with_host_compiler_and_end_with_distribution() {
    let stages = bootstrap_stages();

    assert_eq!(
        stages.first().map(|stage| stage.kind),
        Some(StageKind::HostCompiler)
    );
    assert_eq!(
        stages.last().map(|stage| stage.kind),
        Some(StageKind::Distribution)
    );
}

#[test]
fn bootstrap_stages_include_std_runtime_and_self_hosting_examples() {
    let stages = bootstrap_stages();

    assert!(stages.iter().any(|stage| stage.name == "std"));
    assert!(stages.iter().any(|stage| stage.name == "runtime"));
    assert!(stages
        .iter()
        .any(|stage| stage.name == "self-hosting-examples"));
}
