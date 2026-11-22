use super::Commands;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::ops::{self, CountedQueue as _};
use crate::test_gpu::headless_or_skip;
use std::cell::Cell;
use std::rc::Rc;

fn uniform(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Twice"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// The panic message of `f`, or `None` when it returns.
fn panic_of(f: impl FnOnce()) -> Option<String> {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).err()?;
    let text = caught.downcast_ref::<String>().cloned();
    Some(text.unwrap_or_default())
}

#[test]
fn gpu_a_range_written_twice_while_one_encoder_records_is_refused() {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(r) = headless_or_skip(4, 4) else {
        return;
    };
    let buffer = uniform(&r.device);
    let message = panic_of(|| {
        let _commands = Commands::new(&r.device, &r.queue, "Twice");
        r.queue.counted_write_buffer(&buffer, 0, &[0; 16]);
        r.queue.counted_write_buffer(&buffer, 8, &[1; 16]);
    })
    .expect("a second write to an overlapping range while one encoder records");
    assert!(message.contains("bytes 8..24"), "{message}");
    assert!(
        message.contains("commands_tests.rs"),
        "names both writes: {message}"
    );
    assert!(
        message.contains("Commands::flush"),
        "says what to do: {message}"
    );
    assert!(!super::recording(), "the dropped recording closed");
}

#[test]
fn gpu_a_twice_written_range_names_the_stages_not_the_wrapper() {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(r) = headless_or_skip(4, 4) else {
        return;
    };
    let usage = wgpu::BufferUsages::STORAGE;
    let buffer = GrowBuffer::new(&r.device, "Twice", usage);
    let message = panic_of(|| {
        let _commands = Commands::new(&r.device, &r.queue, "Twice");
        buffer.write_at(&r.queue, 0, &[0; 16]);
        buffer.write_at(&r.queue, 0, &[1; 16]);
    })
    .expect("a range written twice through a wrapper");
    assert!(message.contains("commands_tests.rs"), "{message}");
    assert!(!message.contains("grow_buffer.rs"), "{message}");
}

#[test]
fn gpu_a_write_after_a_flush_or_to_another_range_or_outside_a_recording_passes() {
    let Some(r) = headless_or_skip(4, 4) else {
        return;
    };
    let buffer = uniform(&r.device);
    let write = |at: u64| r.queue.counted_write_buffer(&buffer, at, &[0; 16]);
    write(0);
    write(0);
    let mut commands = Commands::new(&r.device, &r.queue, "Once");
    write(0);
    write(16);
    commands.flush();
    write(0);
    commands.send();
    write(0);
    assert!(!super::recording());
}

#[test]
fn gpu_one_recording_is_one_submit_and_its_maps_wait_for_it() {
    let Some(r) = headless_or_skip(4, 4) else {
        return;
    };
    let _ = ops::take();
    let ran = Rc::new(Cell::new(false));
    let mut commands = Commands::new(&r.device, &r.queue, "Maps");
    let seen = Rc::clone(&ran);
    commands.after_send(move || seen.set(true));
    let tag = super::pair_tag();
    commands
        .encoder()
        .clear_buffer(&uniform(&r.device), 0, None);
    assert!(!ran.get(), "a map before its copy is submitted");
    assert_eq!(
        super::pair_tag(),
        tag,
        "every pair taken meanwhile shares one tag"
    );
    commands.send();
    assert!(ran.get());
    assert_ne!(super::pair_tag(), tag, "the recording's tag ends with it");
    assert_eq!(ops::take().submits, 1);
}
