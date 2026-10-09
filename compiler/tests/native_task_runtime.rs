// Exercise the source embedded in generated native programs directly so that
// thread-start and capacity failures can be injected deterministically.
const __TOK_STACK_BYTES: usize = 64 * 1024;
trait __TokRender {
    fn tok_render(&self) -> String;
}
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/native_runtime/tasks.rs.txt"
));

#[test]
fn failed_thread_start_returns_a_repeatable_task_error() {
    let task: __TokTask<i32> = __tok_task_from_spawn(Err(std::io::Error::other("injected")));
    assert_eq!(task.tok_render(), "<task>");
    assert!(matches!(
        __tok_join(task.clone()),
        Err(__TokTaskError::Failed)
    ));
    assert!(matches!(__tok_join(task), Err(__TokTaskError::Failed)));
}

#[test]
fn capacity_failure_does_not_consume_a_slot() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let running = __tok_spawn_limited(move || receiver.recv().unwrap(), 1);
    let denied = __tok_spawn_limited(|| 99, 1);
    assert!(matches!(__tok_join(denied), Err(__TokTaskError::Failed)));
    sender.send(7).unwrap();
    assert!(matches!(__tok_join(running), Ok(7)));
    let next = __tok_spawn_limited(|| 8, 1);
    assert!(matches!(__tok_join(next), Ok(8)));
}
