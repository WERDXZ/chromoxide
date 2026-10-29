//! Opt-in factorial experiment using the production Base16 recipe factories.
use std::num::{NonZeroU64, NonZeroUsize};
use std::{fs, io::Write, path::Path, time::Instant};

use chromoxide::{HueDomain, ImageCap, Oklab, PaletteProblem, SolveConfig, Term, WeightedSample};
use chromoxide_image::{PreparedImage, load_image_from_path, prepare_image};
use rand::{SeedableRng, rngs::ChaCha8Rng};

use super::super::priors;

mod anchors;
mod semantics;
mod staged;

const VARIANTS: [(&str, bool, bool); 4] = [
    ("baseline", false, false),
    ("linear-support", true, false),
    ("relaxed-accents", false, true),
    ("relaxed-linear", true, true),
];

const COVERAGE_VARIANTS: [(&str, bool, bool); 4] = [
    ("relaxed-linear", true, true),
    ("accent-cover", true, true),
    ("anchors-with-priors", true, true),
    ("source-anchors", true, true),
];

fn make_problem(
    samples: &[WeightedSample],
    cap: Option<&ImageCap>,
    linear: bool,
    relaxed: bool,
    config: SolveConfig,
) -> PaletteProblem {
    let mut slots = super::slots();
    if relaxed {
        for slot in &mut slots[8..] {
            slot.domain.hue = HueDomain::Any;
            slot.domain.chroma.min = 0.0;
        }
    }
    let mut terms = super::terms();
    terms.extend(priors::base16_terms(samples, &slots));
    if linear {
        for weighted in &mut terms {
            if let Term::Support(term) = &mut weighted.term {
                term.beta = term.tau;
                term.epsilon = 0.0;
            }
        }
    }
    PaletteProblem {
        slots,
        samples: samples.to_vec(),
        image_cap: cap.cloned(),
        terms,
        config,
    }
}

fn experiment_config() -> SolveConfig {
    SolveConfig {
        seed_count: NonZeroUsize::new(8).unwrap(),
        max_iters: NonZeroU64::new(120).unwrap(),
        ..Default::default()
    }
}

#[test]
fn factorial_variants_keep_lightness_roles_and_existing_term_weights() {
    let samples = vec![WeightedSample::new(
        chromoxide::Oklch {
            l: 0.5,
            c: 0.05,
            h: 1.0,
        }
        .to_oklab(),
        1.0,
        0.5,
    )];
    let baseline = make_problem(&samples, None, false, false, experiment_config());
    for (_, linear, relaxed) in VARIANTS {
        let variant = make_problem(&samples, None, linear, relaxed, experiment_config());
        for (index, (before, after)) in baseline.slots.iter().zip(&variant.slots).enumerate() {
            assert_eq!(before.name, after.name);
            assert_eq!(before.domain.lightness, after.domain.lightness);
            assert_eq!(before.domain.cap_policy, after.domain.cap_policy);
            assert_eq!(before.domain.chroma.max, after.domain.chroma.max);
            if relaxed && index >= 8 {
                assert_eq!(after.domain.chroma.min, 0.0);
                assert_eq!(after.domain.hue, HueDomain::Any);
            } else {
                assert_eq!(before.domain, after.domain);
            }
        }
        for (before, after) in baseline.terms.iter().zip(&variant.terms) {
            assert_eq!(before.weight, after.weight);
            assert_eq!(before.name, after.name);
            if let Term::Support(term) = &after.term {
                assert_eq!(term.beta, if linear { term.tau } else { 0.10 });
                assert_eq!(term.epsilon, if linear { 0.0 } else { 1.0e-4 });
            }
        }
        assert_eq!(baseline.terms.len(), variant.terms.len());
    }
}

fn write_ppm(path: &Path, width: usize, height: usize, pixels: &[[u8; 3]]) {
    assert_eq!(pixels.len(), width * height);
    let mut data = format!("P6\n{width} {height}\n255\n").into_bytes();
    data.extend(pixels.iter().flatten());
    fs::write(path, data).unwrap();
}

fn fixtures(out: &Path) -> Vec<(String, std::path::PathBuf)> {
    let names = [
        "gray",
        "gray-red-small",
        "cool80-warm20",
        "cool20-warm80",
        "gray-red-pixel",
        "gray-green-small",
        "gray-red-green",
    ];
    let mut paths = Vec::new();
    for (case, name) in names.into_iter().enumerate() {
        let mut pixels = Vec::new();
        for y in 0..64 {
            for x in 0..80 {
                let rgb = match case {
                    0 => [128, 128, 128],
                    1 if (36..44).contains(&x) && (28..36).contains(&y) => [210, 40, 35],
                    1 => [125, 130, 135],
                    2 if x < 64 => [90, 120, 145],
                    3 if x < 16 => [90, 120, 145],
                    4 if x == 40 && y == 32 => [210, 40, 35],
                    4 => [125, 130, 135],
                    5 | 6 if (36..44).contains(&x) && (28..36).contains(&y) => [45, 150, 85],
                    6 if (20..28).contains(&x) && (28..36).contains(&y) => [210, 40, 35],
                    5 | 6 => [125, 130, 135],
                    _ => [210, 115, 45],
                };
                pixels.push(rgb);
            }
        }
        let path = out.join(format!("{name}.ppm"));
        write_ppm(&path, 80, 64, &pixels);
        paths.push((name.into(), path));
    }
    if let Some(input) = std::env::var_os("CHROMOXIDE_FIDELITY_IMAGES") {
        let mut photos = Vec::new();
        for path in std::env::split_paths(&input) {
            if path.is_dir() {
                photos.extend(
                    fs::read_dir(path)
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|p| {
                            p.extension().and_then(|v| v.to_str()).is_some_and(|ext| {
                                matches!(
                                    ext.to_ascii_lowercase().as_str(),
                                    "jpg" | "jpeg" | "png" | "webp"
                                )
                            })
                        }),
                );
            } else {
                photos.push(path);
            }
        }
        photos.sort();
        photos.dedup();
        for (i, path) in photos.into_iter().enumerate() {
            paths.push((format!("photo-{i:02}"), path));
        }
    }
    if let Ok(selected) = std::env::var("CHROMOXIDE_FIDELITY_CASES") {
        paths.retain(|(name, _)| selected.split(',').any(|case| case == name));
        assert!(!paths.is_empty(), "CASES matched no input");
    }
    paths
}

fn nearest_distance2(lab: Oklab, colors: impl Iterator<Item = Oklab>) -> f64 {
    colors
        .map(|color| lab.distance2(color))
        .fold(f64::INFINITY, f64::min)
}

fn cover_scale_audit(out: &Path) {
    let mut file = fs::File::create(out.join("cover-scale.csv")).unwrap();
    writeln!(
        file,
        "distance,current_huber_of_squared,squared,huber_of_distance"
    )
    .unwrap();
    for d in [0.005, 0.01, 0.025, 0.05, 0.1, 0.2] {
        writeln!(
            file,
            "{d},{:.12},{:.12},{:.12}",
            chromoxide::util::pseudo_huber(d * d, 0.03),
            d * d,
            chromoxide::util::pseudo_huber(d, 0.03)
        )
        .unwrap();
    }
}

#[test]
#[ignore = "writes fidelity artifacts and runs repeated multi-start solves"]
fn photo_fidelity_comparison() {
    let out = std::env::var_os("CHROMOXIDE_FIDELITY_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/photo-fidelity")
        });
    fs::create_dir_all(&out).unwrap();
    let phase = std::env::var("CHROMOXIDE_FIDELITY_PHASE").unwrap_or_else(|_| "factorial".into());
    let mut variants = match phase.as_str() {
        "factorial" => VARIANTS.to_vec(),
        "coverage" => COVERAGE_VARIANTS.to_vec(),
        "semantics" => vec![
            ("source-anchors", true, true),
            ("semantic-anchors", true, true),
        ],
        "photos" => vec![
            ("baseline", false, false),
            ("relaxed-linear", true, true),
            ("source-anchors", true, true),
            ("semantic-anchors", true, true),
        ],
        "protected" => vec![
            ("semantic-anchors", true, true),
            ("protected-anchors", true, true),
        ],
        "staged" => vec![
            ("source-anchors", true, true),
            ("protected-anchors", true, true),
            ("staged-anchors", true, true),
            ("staged-decor", true, true),
        ],
        _ => panic!("PHASE must be factorial, coverage, semantics, photos, protected, or staged"),
    };
    if let Ok(selected) = std::env::var("CHROMOXIDE_FIDELITY_VARIANTS") {
        variants.retain(|(name, _, _)| selected.split(',').any(|v| v == *name));
        assert!(!variants.is_empty(), "VARIANTS matched no variant");
    }
    let mut solve_config = experiment_config();
    if let Ok(starts) = std::env::var("CHROMOXIDE_FIDELITY_STARTS") {
        solve_config.seed_count = starts.parse().expect("STARTS must be positive");
    }
    if let Ok(iters) = std::env::var("CHROMOXIDE_FIDELITY_ITERS") {
        solve_config.max_iters = iters.parse().expect("ITERS must be positive");
    }
    // Explicit replay evaluates a new adapter on existing source-only scenes.
    // It is not a timing benchmark and never silently falls back to a solve.
    let scene_replay = std::env::var_os("CHROMOXIDE_FIDELITY_SCENES").map(std::path::PathBuf::from);
    if let Some(path) = &scene_replay {
        assert!(
            variants
                .iter()
                .all(|(name, _, _)| name.starts_with("staged-")),
            "SCENES requires staged variants only"
        );
        assert_ne!(
            fs::canonicalize(path).unwrap(),
            fs::canonicalize(&out).unwrap(),
            "SCENES must not be the output directory"
        );
        let config = fs::read_to_string(path.join("run-config.txt")).unwrap();
        assert!(
            config.contains(&format!("{solve_config:#?}"))
                && config.contains("image seed = [42; 32]")
                && config.contains("solve seeds = [7; 32], [23; 32], [42; 32]"),
            "SCENES config/seeds do not match"
        );
    }
    fs::write(
        out.join("run-config.txt"),
        format!(
            "phase = {phase}\nvariants = {variants:?}\nimage seed = [42; 32]\nsolve seeds = [7; 32], [23; 32], [42; 32]\nscene replay = {scene_replay:?}\nStaged solver fields describe the source solve; projection has no convergence flag.\n{solve_config:#?}\n"
        ),
    )
    .unwrap();
    cover_scale_audit(&out);
    let mut metrics = fs::File::create(out.join("metrics.csv")).unwrap();
    writeln!(metrics, "input,variant,seed,samples,seconds,objective,converged,iterations,gradient_norm,source_mean_chroma,accent_chroma_raw,accent_chroma_hex,coverage_rmse_hex,accent_source_distance_hex,max_export_drift,bg_fg_contrast_hex,out_of_gamut,accent_floor_hits,distinct_accents_hex,adaptation_seconds").unwrap();
    let mut colors = fs::File::create(out.join("colors.csv")).unwrap();
    let mut assignments = fs::File::create(out.join("assignments.csv")).unwrap();
    let mut provenance = fs::File::create(out.join("provenance.csv")).unwrap();
    writeln!(provenance, "input,variant,slot,origin,source_l,source_a,source_b,source_area,target_l,target_c,target_h").unwrap();
    let mut roles = fs::File::create(out.join("roles.csv")).unwrap();
    writeln!(roles, "input,variant,seed,role,slot,origin,hex,contrast_hex,nearest_role_distance_hex,hue_ok,chroma_ok,passes").unwrap();
    let mut semantic_failures = Vec::new();
    writeln!(
        assignments,
        "input,variant,slot,source_l,source_a,source_b,source_area"
    )
    .unwrap();
    writeln!(
        colors,
        "input,variant,seed,slot,hex,l,c,h,hex_l,hex_a,hex_b"
    )
    .unwrap();
    let image_config = crate::config::default_image_config();
    let inputs = fixtures(&out);
    let manifest: std::collections::BTreeMap<_, _> = inputs
        .iter()
        .map(|(name, path)| (name, path.to_string_lossy().into_owned()))
        .collect();
    fs::write(out.join("inputs.toml"), toml::to_string(&manifest).unwrap()).unwrap();
    for (name, path) in inputs {
        let image = load_image_from_path(&path).unwrap();
        image
            .thumbnail(256, 256)
            .save(out.join(format!("{name}.png")))
            .unwrap();
        let prepared = prepare_image(&image, &image_config.preprocess).unwrap();
        let support = chromoxide_image::prepare_support_from_image_with_rng(
            &image,
            &image_config,
            &mut ChaCha8Rng::from_seed([42; 32]),
        )
        .unwrap();
        fs::write(
            out.join(format!("{name}-support.toml")),
            toml::to_string(&support).unwrap(),
        )
        .unwrap();
        if let Some(replay) = &scene_replay {
            assert_eq!(
                fs::read(replay.join(format!("{name}-support.toml"))).unwrap(),
                fs::read(out.join(format!("{name}-support.toml"))).unwrap(),
                "SCENES support differs for {name}"
            );
        }
        let mut source = fs::File::create(out.join(format!("{name}-samples.csv"))).unwrap();
        writeln!(source, "hex,weight,saliency").unwrap();
        for sample in &support.samples {
            writeln!(
                source,
                "{},{:.12},{:.12}",
                crate::filter::apply("hex", sample.lab.to_oklch()).unwrap(),
                sample.weight,
                sample.saliency
            )
            .unwrap();
        }
        // Both staged ablations use the identical source solve. Cache only
        // within this image; reported pipeline time still includes that solve.
        let mut source_runs =
            std::collections::BTreeMap::<u8, (chromoxide::PaletteSolution, f64)>::new();
        for &(variant, linear, relaxed) in &variants {
            let is_staged = variant.starts_with("staged-");
            let mut problem = make_problem(
                &support.samples,
                support.image_cap.as_ref(),
                linear,
                relaxed,
                solve_config.clone(),
            );
            let staged_assignments = if is_staged {
                semantics::apply_protected(&mut problem.clone())
            } else {
                Vec::new()
            };
            if variant == "accent-cover" {
                // Scope-only control: preserve Cover's existing weight/scale.
                problem.terms.push(chromoxide::WeightedTerm {
                    name: Some("accent-cover".into()),
                    weight: 5.0,
                    term: Term::Cover(chromoxide::CoverTerm {
                        slots: (8..16).collect(),
                        tau: 0.02,
                        delta: 0.03,
                    }),
                });
            }
            if is_staged || matches!(variant, "source-anchors" | "anchors-with-priors") {
                for (slot, source) in anchors::apply(&mut problem, variant == "anchors-with-priors")
                {
                    writeln!(
                        assignments,
                        "{name},{variant},{slot},{:.12},{:.12},{:.12},{:.12}",
                        source.lab.l, source.lab.a, source.lab.b, source.weight
                    )
                    .unwrap();
                }
            }
            let mut semantic_assignments = if variant == "semantic-anchors" {
                semantics::apply(&mut problem)
            } else if variant == "protected-anchors" {
                semantics::apply_protected(&mut problem)
            } else if is_staged {
                staged_assignments
            } else {
                Vec::new()
            };
            if is_staged {
                for assignment in &mut semantic_assignments {
                    if let Some(source) = &assignment.source {
                        assignment.target.l = source.lab.l;
                    }
                }
            }
            for assignment in &semantic_assignments {
                let evidence = assignment.source.as_ref().map_or(",,,".to_owned(), |s| {
                    format!(
                        "{:.12},{:.12},{:.12},{:.12}",
                        s.lab.l, s.lab.a, s.lab.b, s.weight
                    )
                });
                let origin = if assignment.source.is_some() {
                    "source-adapted"
                } else {
                    "synthetic"
                };
                let c = assignment.target;
                writeln!(
                    provenance,
                    "{name},{variant},{},{origin},{evidence},{:.12},{:.12},{:.12}",
                    assignment.slot, c.l, c.c, c.h
                )
                .unwrap();
            }
            for seed in [7, 23, 42] {
                let solve = || {
                    if let Some(replay) = &scene_replay {
                        let saved = fs::read_to_string(
                            replay.join(format!("{name}-source-anchors-{seed}.toml")),
                        )
                        .unwrap();
                        let solution: chromoxide::PaletteSolution = toml::from_str(&saved).unwrap();
                        assert_eq!(solution.colors_lch.len(), 16);
                        assert_eq!(
                            solution.solver_diagnostics.seed_count,
                            solve_config.seed_count.get()
                        );
                        return (solution, f64::NAN);
                    }
                    let start = Instant::now();
                    let solution = chromoxide::solve_with_seed(&problem, [seed; 32]).unwrap();
                    (solution, start.elapsed().as_secs_f64())
                };
                let (solution, source_seconds) = if is_staged || variant == "source-anchors" {
                    source_runs.entry(seed).or_insert_with(solve).clone()
                } else {
                    solve()
                };
                let scene = staged::ScenePalette {
                    colors: solution.colors_lch.clone(),
                    samples: support.samples.clone(),
                };
                let start = Instant::now();
                let theme = is_staged.then(|| {
                    staged::adapt(&scene, &semantic_assignments, variant == "staged-decor").unwrap()
                });
                let adaptation_seconds = if is_staged {
                    start.elapsed().as_secs_f64()
                } else {
                    0.0
                };
                let seconds = source_seconds + adaptation_seconds;
                let colors_lch = theme.as_ref().map_or(&solution.colors_lch, |t| &t.colors);
                let colors_lab: Vec<_> = colors_lch.iter().map(|c| c.to_oklab()).collect();
                #[derive(serde::Serialize)]
                struct StagedArtifact<'a> {
                    source_solution: &'a chromoxide::PaletteSolution,
                    scene: &'a staged::ScenePalette,
                    theme: &'a staged::ThemePalette,
                }
                let artifact = theme.as_ref().map_or_else(
                    || toml::to_string(&solution).unwrap(),
                    |theme| {
                        toml::to_string(&StagedArtifact {
                            source_solution: &solution,
                            scene: &scene,
                            theme,
                        })
                        .unwrap()
                    },
                );
                fs::write(out.join(format!("{name}-{variant}-{seed}.toml")), artifact).unwrap();
                let rgb: Vec<_> = colors_lch
                    .iter()
                    .map(|&c| {
                        let (r, g, b) = crate::filter::srgb_u8(c);
                        [r, g, b]
                    })
                    .collect();
                let strip = out.join(format!("{name}-{variant}-{seed}.ppm"));
                write_ppm(&strip, rgb.len(), 1, &rgb);
                let displayed =
                    prepare_image(&load_image_from_path(&strip).unwrap(), &Default::default())
                        .unwrap();
                for role in semantics::ROLES {
                    let pixel = displayed.pixels[role.slot];
                    let c = pixel.lab.to_oklch();
                    let role_contrast = chromoxide::convert::contrast_ratio(
                        pixel.luminance,
                        displayed.pixels[0].luminance,
                    );
                    let distance = semantics::ROLES
                        .iter()
                        .filter(|r| r.slot != role.slot)
                        .map(|r| pixel.lab.distance2(displayed.pixels[r.slot].lab).sqrt())
                        .fold(f64::INFINITY, f64::min);
                    let hue_ok = role.hue().contains(c.h);
                    let chroma_ok = c.c >= 0.03;
                    let passes = hue_ok
                        && chroma_ok
                        && role_contrast >= semantics::MIN_CONTRAST
                        && distance >= semantics::MIN_ROLE_DISTANCE;
                    let origin = semantic_assignments
                        .iter()
                        .find(|a| a.slot == role.slot)
                        .map_or("unassigned", |a| {
                            if a.source.is_some() {
                                "source-adapted"
                            } else {
                                "synthetic"
                            }
                        });
                    writeln!(roles, "{name},{variant},{seed},{},{},{origin},{},{role_contrast:.6},{distance:.9},{hue_ok},{chroma_ok},{passes}",
                        role.name, role.slot, crate::filter::apply("hex", colors_lch[role.slot]).unwrap()).unwrap();
                    let enforce_roles = is_staged
                        || variant == "protected-anchors"
                        || (variant == "semantic-anchors"
                            && !matches!(phase.as_str(), "protected" | "staged"));
                    if enforce_roles && !passes {
                        semantic_failures.push(format!("{name}/{seed}/{}: contrast {role_contrast:.3}, distance {distance:.4}, hue {hue_ok}, chroma {chroma_ok}", role.name));
                    }
                }
                if is_staged || variant == "protected-anchors" {
                    for slot in semantics::TEXT_SLOTS {
                        let ratio = chromoxide::convert::contrast_ratio(
                            displayed.pixels[slot].luminance,
                            displayed.pixels[0].luminance,
                        );
                        if ratio < semantics::MIN_CONTRAST {
                            semantic_failures
                                .push(format!("{name}/{seed}/text-{slot}: contrast {ratio:.3}"));
                        }
                    }
                }
                for (index, (lch, pixel)) in colors_lch.iter().zip(&displayed.pixels).enumerate() {
                    writeln!(colors, "{name},{variant},{seed},{index},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12}",
                        crate::filter::apply("hex", *lch).unwrap(), lch.l,lch.c,lch.h,
                        pixel.lab.l,pixel.lab.a,pixel.lab.b).unwrap();
                }
                let (coverage, source_chroma, source_distance) =
                    source_metrics(&prepared, &displayed);
                let accent_chroma_raw = colors_lch[8..].iter().map(|c| c.c).sum::<f64>() / 8.0;
                let accent_chroma_hex = displayed.pixels[8..]
                    .iter()
                    .map(|p| p.lab.to_oklch().c)
                    .sum::<f64>()
                    / 8.0;
                let export_drift = colors_lab
                    .iter()
                    .zip(&displayed.pixels)
                    .map(|(a, b)| a.distance2(b.lab).sqrt())
                    .fold(0.0, f64::max);
                let out_of_gamut = colors_lab
                    .iter()
                    .filter(|&&lab| {
                        let rgb = chromoxide::convert::oklab_to_linear_srgb(lab);
                        [rgb.r, rgb.g, rgb.b]
                            .iter()
                            .any(|c| *c < -1.0e-8 || *c > 1.0 + 1.0e-8)
                    })
                    .count();
                let floor_hits = problem.slots[8..]
                    .iter()
                    .zip(&colors_lch[8..])
                    .filter(|(s, c)| c.c - s.domain.chroma.min < 1.0e-4)
                    .count();
                let contrast = chromoxide::convert::contrast_ratio(
                    displayed.pixels[0].luminance,
                    displayed.pixels[5].luminance,
                );
                let distinct = rgb[8..]
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                // For staged rows solver fields describe the source solve;
                // the final text projection has no optimizer/convergence flag.
                writeln!(metrics, "{name},{variant},{seed},{},{seconds:.6},{:.12},{},{},{:.12},{source_chroma:.9},{accent_chroma_raw:.9},{accent_chroma_hex:.9},{coverage:.9},{source_distance:.9},{export_drift:.9},{contrast:.6},{out_of_gamut},{floor_hits},{distinct},{adaptation_seconds:.6}",
                    support.samples.len(), solution.objective, solution.converged,
                    solution.solver_diagnostics.iterations,
                    solution.solver_diagnostics.final_gradient_norm.unwrap_or(f64::NAN)).unwrap();
                metrics.flush().unwrap();
                println!(
                    "{name:18} {variant:16} seed={seed:2} C={accent_chroma_hex:.4} source-gap={source_distance:.4} coverage={coverage:.4} {seconds:.2}s"
                );
            }
        }
    }
    println!("Artifacts: {}", out.display());
    assert!(
        semantic_failures.is_empty(),
        "Final HEX theme checks failed:\n{}",
        semantic_failures.join("\n")
    );
}

fn source_metrics(prepared: &PreparedImage, displayed: &PreparedImage) -> (f64, f64, f64) {
    let mut mass = 0.0;
    let mut coverage = 0.0;
    let mut chroma = 0.0;
    for &index in &prepared.valid_indices {
        let px = prepared.pixels[index];
        mass += px.alpha;
        coverage += px.alpha * nearest_distance2(px.lab, displayed.pixels.iter().map(|p| p.lab));
        chroma += px.alpha * px.lab.to_oklch().c;
    }
    let accent_source_distance = displayed.pixels[8..]
        .iter()
        .map(|p| {
            nearest_distance2(
                p.lab,
                prepared
                    .valid_indices
                    .iter()
                    .map(|&i| prepared.pixels[i].lab),
            )
            .sqrt()
        })
        .sum::<f64>()
        / 8.0;
    (
        (coverage / mass).sqrt(),
        chroma / mass,
        accent_source_distance,
    )
}
