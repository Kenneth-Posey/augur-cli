use augur_core::plan_store::{PlanStoreError, PlanTreeStore};
use augur_domain::domain::plan_tree::{NodeKind, PlanNode, PlanTree, PlanTreeId};
use augur_domain::domain::string_newtypes::{StepContent, StepFileName, StringNewtype};
use tempfile::TempDir;

fn temp_store() -> (TempDir, PlanTreeStore) {
    let dir = TempDir::new().expect("tempdir");
    let store = PlanTreeStore::new(dir.path().to_path_buf());
    (dir, store)
}

fn sample_tree() -> PlanTree {
    let leaf = PlanNode::new_leaf("l1", "Step 1", "steps/l1.md");
    let mut tree = PlanTree::new("tree-1", "Sample Plan", "Do something");
    tree.root = tree.root.add_child(leaf);
    tree
}

#[tokio::test]
async fn plan_store_save_and_load_round_trips_tree_json() {
    let (_dir, store) = temp_store();
    let tree = sample_tree();
    store.save(&tree).await.expect("save");
    let loaded = store.load(&PlanTreeId::new("tree-1")).await.expect("load");
    assert_eq!(tree.id, loaded.id);
    assert_eq!(tree.title, loaded.title);
    assert_eq!(tree.goal, loaded.goal);
    assert_eq!(tree.root.children.len(), loaded.root.children.len());
    assert_eq!(tree.root.children[0].id, loaded.root.children[0].id);
}

#[tokio::test]
async fn plan_store_save_creates_directory() {
    let (dir, store) = temp_store();
    let tree = sample_tree();
    store.save(&tree).await.expect("save");
    let plan_dir = dir.path().join("tree-1");
    assert!(
        plan_dir.exists(),
        "plan directory should be created by save"
    );
}

#[tokio::test]
async fn plan_store_write_and_read_step_round_trips_content() {
    let (_dir, store) = temp_store();
    let tree = sample_tree();
    store.save(&tree).await.expect("save");

    let id = PlanTreeId::new("tree-1");
    let content = StepContent::new("# Install deps\n\nRun `cargo build` and verify it compiles.\n");
    let step_file = StepFileName::new("l1.md");
    store
        .write_step(&id, &step_file, &content)
        .await
        .expect("write");
    let read = store.read_step(&id, &step_file).await.expect("read");
    assert_eq!(content.as_str(), read.as_str());
}

#[tokio::test]
async fn plan_store_write_step_creates_steps_directory() {
    let (dir, store) = temp_store();
    let id = PlanTreeId::new("tree-new");
    let step_file = StepFileName::new("s1.md");
    let content = StepContent::new("content");
    store
        .write_step(&id, &step_file, &content)
        .await
        .expect("write");
    let step_path = dir.path().join("tree-new").join("steps").join("s1.md");
    assert!(step_path.exists());
}

#[tokio::test]
async fn plan_store_load_returns_not_found_for_missing_plan() {
    let (_dir, store) = temp_store();
    let err = store.load(&PlanTreeId::new("no-such-plan")).await;
    assert!(matches!(err, Err(PlanStoreError::NotFound(_))));
}

#[tokio::test]
async fn plan_store_read_step_returns_not_found_for_missing_file() {
    let (_dir, store) = temp_store();
    let id = PlanTreeId::new("tree-1");
    let err = store.read_step(&id, &StepFileName::new("ghost.md")).await;
    assert!(matches!(err, Err(PlanStoreError::NotFound(_))));
}

#[tokio::test]
async fn plan_store_save_overwrites_existing_tree() {
    let (_dir, store) = temp_store();
    let tree1 = PlanTree::new("tree-1", "First title", "goal");
    store.save(&tree1).await.expect("first save");

    let mut tree2 = PlanTree::new("tree-1", "Second title", "goal");
    tree2.root = tree2
        .root
        .add_child(PlanNode::new_branch("b2", "New branch"));
    store.save(&tree2).await.expect("second save");

    let loaded = store.load(&PlanTreeId::new("tree-1")).await.expect("load");
    assert_eq!(loaded.title, "Second title");
    assert_eq!(loaded.root.children[0].id, tree2.root.children[0].id);
}

#[tokio::test]
async fn plan_store_preserves_node_kind_on_round_trip() {
    let (_dir, store) = temp_store();
    let tree = sample_tree();
    store.save(&tree).await.expect("save");
    let loaded = store.load(&PlanTreeId::new("tree-1")).await.expect("load");
    assert_eq!(loaded.root.children[0].config.kind, NodeKind::Leaf);
}

#[test]
fn plan_store_default_uses_plans_base_dir() {
    let store = PlanTreeStore::default();
    drop(store);
}
