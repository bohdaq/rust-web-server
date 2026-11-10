use super::*;
use crate::test_env::TempPath;

fn project(name: &str) -> TempPath {
    let dir = TempPath::new(&format!("gen_deploy_{name}"));
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"Notes_App\"\n").unwrap();
    std::fs::write(dir.join("rws.app.toml"), "[app]\nname = \"Notes App!\"\n").unwrap();
    dir
}

#[test]
fn names_become_dns_labels() {
    assert_eq!("notes-app", k8s_name("Notes App!"));
    assert_eq!("a-b", k8s_name("--A__b--"));
    assert_eq!("app", k8s_name("!!!"));
    assert_eq!(63, k8s_name(&"x".repeat(100)).len());
}

#[test]
fn writes_the_manifests_once() {
    let dir = project("write");
    let mut spec = DeploySpec::new("ghcr.io/acme/notes:1.0.0");
    spec.port = 8080;
    spec.replicas = 3;
    let plan = plan_deploy(&*dir, &spec).unwrap();
    assert_eq!(6, plan.changes.len());
    plan.apply(&*dir).unwrap();
    let deployment = std::fs::read_to_string(dir.join("deploy/k8s/deployment.yaml")).unwrap();
    for needle in ["name: notes-app", "replicas: 3", "image: ghcr.io/acme/notes:1.0.0", "containerPort: 8080", "path: /startupz", "path: /readyz", "path: /livez", "terminationGracePeriodSeconds: 35", "fieldPath: metadata.namespace", "--port 8080 --replicas 3"] {
        assert!(deployment.contains(needle), "{needle} in:\n{deployment}");
    }
    let hpa = std::fs::read_to_string(dir.join("deploy/k8s/hpa.yaml")).unwrap();
    assert!(hpa.contains("minReplicas: 3") && hpa.contains("maxReplicas: 10"));
    assert!(std::fs::read_to_string(dir.join("deploy/k8s/kustomization.yaml")).unwrap().contains("- deployment.yaml"));
    // Again: nothing changes; something else: a conflict.
    assert!(plan_deploy(&*dir, &spec).unwrap().is_unchanged());
    assert!(plan_deploy(&*dir, &DeploySpec::new("other:2")).unwrap().has_conflicts());
}

#[test]
fn refusals() {
    let dir = project("refuse");
    assert!(plan_deploy(&*dir, &DeploySpec::new("")).is_err());
    assert!(plan_deploy(&*dir, &DeploySpec::new("a b")).is_err());
    let mut zero = DeploySpec::new("img");
    zero.replicas = 0;
    assert!(plan_deploy(&*dir, &zero).is_err());
    let mut outside = DeploySpec::new("img");
    outside.dir = "../elsewhere".into();
    assert!(plan_deploy(&*dir, &outside).is_err());
    assert!(plan_deploy("/no/such/project", &DeploySpec::new("img")).is_err());
}

#[test]
fn the_command_line() {
    let dir = project("cli");
    let run = |args: &[&str]| {
        let mut a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        a.extend(["--dir".to_string(), dir.to_str().unwrap().to_string()]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = crate::generator::run(&a, &mut out, &mut err);
        (code, String::from_utf8_lossy(&out).to_string() + &String::from_utf8_lossy(&err))
    };
    assert_eq!(crate::generator::EXIT_USAGE, run(&["deploy"]).0);
    assert_eq!(crate::generator::EXIT_USAGE, run(&["deploy", "--image", "x", "--port", "99999"]).0);
    assert_eq!(crate::generator::EXIT_USAGE, run(&["deploy", "extra", "--image", "x"]).0);
    assert_eq!(crate::generator::EXIT_USAGE, run(&["model", "Post", "a:i64", "--image", "x"]).0);
    let (code, text) = run(&["deploy", "--image", "ghcr.io/acme/notes:1", "--out", "k8s", "--dry-run"]);
    assert_eq!(crate::generator::EXIT_OK, code, "{text}");
    assert!(text.contains("k8s/deployment.yaml") && !dir.join("k8s").exists(), "{text}");
    assert_eq!(crate::generator::EXIT_OK, run(&["deploy", "--image", "ghcr.io/acme/notes:1", "--out", "k8s"]).0);
    assert!(dir.join("k8s/service.yaml").exists());
}
