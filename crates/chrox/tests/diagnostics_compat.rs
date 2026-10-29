use chromoxide::SolverDiagnostics;

#[test]
fn old_solver_diagnostics_remain_readable_and_new_stop_reasons_roundtrip() {
    let legacy = r#"
seed_count = 1
best_seed_index = 0
converged = false
iterations = 3
final_gradient_norm = 0.2
[[seed_runs]]
seed_index = 0
objective = 0.1
converged = false
iterations = 3
"#;
    let mut diagnostics: SolverDiagnostics = toml::from_str(legacy).unwrap();
    assert!(diagnostics.termination_reason.is_none());
    assert!(diagnostics.seed_runs[0].termination_reason.is_none());
    diagnostics.termination_reason = Some("Line search terminated with: 'test'".into());
    diagnostics.seed_runs[0].termination_reason = diagnostics.termination_reason.clone();
    let encoded = toml::to_string(&diagnostics).unwrap();
    let decoded: SolverDiagnostics = toml::from_str(&encoded).unwrap();
    assert_eq!(decoded.termination_reason, diagnostics.termination_reason);
    assert_eq!(
        decoded.seed_runs[0].termination_reason,
        diagnostics.termination_reason
    );
    assert!(!decoded.converged);
}
