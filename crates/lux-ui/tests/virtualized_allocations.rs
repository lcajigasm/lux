//! Guard the allocation cost of the actual frame builder, not just the
//! number of shaped lines. Counting bytes avoids a hardware-dependent timer.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Duration;

use lux_core::{
    buffer::Buffer, multicursor::MultiCursor, position::Position, selection::Selection,
};
use lux_render::TextLayoutContext;
use lux_ui::{EditorView, EditorViewMetrics, Viewport};

struct CountedAllocator;
thread_local! { static ALLOCATED: Cell<Option<usize>> = const { Cell::new(None) }; }

fn record(bytes: usize) {
    let _ = ALLOCATED.try_with(|total| {
        if let Some(previous) = total.get() {
            total.set(Some(previous.saturating_add(bytes)));
        }
    });
}

// Forward allocations unchanged, counting only the current test thread
// while the frame is built. The allocator itself performs no allocation.
unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

#[test]
fn frame_allocations_are_bounded_by_viewport_with_a_whole_document_selection() {
    let buffer = Buffer::from_bytes("line\n".repeat(2_000_000).as_bytes()).unwrap();
    let cursors = MultiCursor::new(Selection::new(
        Position::document_start(),
        Position::document_end(&buffer),
    ));
    let metrics = EditorViewMetrics {
        line_height: 20.0,
        font_size: 16.0,
        ligatures: false,
        word_wrap: false,
        column_rulers: vec![],
        blink_interval: Duration::from_millis(500),
    };
    let mut view = EditorView::new(
        TextLayoutContext::new(),
        include_bytes!("fixtures/ahem.ttf"),
        metrics,
    )
    .unwrap();
    let viewport = Viewport {
        width: 800.0,
        height: 80.0,
        scroll_x: 0.0,
        scroll_y: 20_000_000.0,
    };
    // Prime font/shaping caches before measuring per-frame work.
    view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);
    ALLOCATED.with(|total| total.set(Some(0)));
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);
    let bytes = ALLOCATED.with(|total| total.replace(None).unwrap());
    assert_eq!(frame.visible_line_range, 1_000_000..1_000_005);
    assert_eq!(frame.selections.len(), 5);
    assert!(
        bytes < 1_000_000,
        "a five-line frame allocated {bytes} bytes"
    );
}
