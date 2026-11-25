use super::*;

fn at(selections: &[(usize, usize)]) -> SelectionSnapshot {
    SelectionSnapshot {
        selections: selections.to_vec(),
        scroll: (0.0, 0.0),
    }
}

#[test]
fn a_change_of_selection_becomes_one_step() {
    let mut history = SelectionHistory::new();
    history.settle(at(&[(0, 0)]), 0);
    history.settle(at(&[(0, 0)]), 0);
    assert!(!history.can_soft_undo());
    history.settle(at(&[(0, 3)]), 0);
    history.settle(at(&[(0, 3), (8, 11)]), 0);

    let back = history.soft_undo(at(&[(0, 3), (8, 11)])).unwrap();
    assert_eq!(back.selections, vec![(0, 3)]);
    let back = history.soft_undo(back).unwrap();
    assert_eq!(back.selections, vec![(0, 0)]);
    assert!(history.soft_undo(back.clone()).is_none());

    let forward = history.soft_redo(back).unwrap();
    assert_eq!(forward.selections, vec![(0, 3)]);
    let forward = history.soft_redo(forward).unwrap();
    assert_eq!(forward.selections, vec![(0, 3), (8, 11)]);
    assert!(history.soft_redo(forward).is_none());
}

#[test]
fn a_new_step_clears_soft_redo() {
    let mut history = SelectionHistory::new();
    history.settle(at(&[(0, 0)]), 0);
    history.settle(at(&[(5, 5)]), 0);
    let back = history.soft_undo(at(&[(5, 5)])).unwrap();
    history.settle(back, 0);
    assert!(history.can_soft_redo());
    history.settle(at(&[(2, 2)]), 0);
    assert!(!history.can_soft_redo());
}

#[test]
fn scroll_alone_is_not_a_step_but_is_remembered() {
    let mut history = SelectionHistory::new();
    history.settle(at(&[(0, 0)]), 0);
    let mut scrolled = at(&[(0, 0)]);
    scrolled.scroll = (0.0, 400.0);
    history.settle(scrolled, 0);
    assert!(!history.can_soft_undo());
    history.settle(at(&[(9, 9)]), 0);
    let back = history.soft_undo(at(&[(9, 9)])).unwrap();
    assert_eq!(back.scroll, (0.0, 400.0));
}

#[test]
fn edits_move_remembered_selections_without_adding_steps() {
    let mut history = SelectionHistory::new();
    history.settle(at(&[(10, 14)]), 0);
    history.settle(at(&[(0, 0)]), 0);
    // Type three chars at the start, then delete chars 11..13 (inside the
    // remembered selection after the shift).
    history.after_edit(
        &[
            Edit::insert(0, "abc"),
            Edit::Delete {
                char_idx: 11,
                text: "xy".into(),
            },
        ],
        at(&[(3, 3)]),
        1,
    );
    history.settle(at(&[(3, 3)]), 1);
    let back = history.soft_undo(at(&[(3, 3)])).unwrap();
    // 10..14 → 13..17 after the insert → 11..15 after deleting 11..13.
    assert_eq!(back.selections, vec![(11, 15)]);
}

#[test]
fn an_unreported_text_change_starts_over() {
    let mut history = SelectionHistory::new();
    history.settle(at(&[(0, 0)]), 0);
    history.settle(at(&[(4, 4)]), 0);
    assert!(history.can_soft_undo());
    history.settle(at(&[(1, 1)]), 7);
    assert!(!history.can_soft_undo());
}

#[test]
fn history_is_bounded() {
    let mut history = SelectionHistory::new();
    for caret in 0..CAPACITY + 10 {
        history.settle(at(&[(caret, caret)]), 0);
    }
    let mut current = at(&[(CAPACITY + 9, CAPACITY + 9)]);
    let mut steps = 0;
    while let Some(back) = history.soft_undo(current.clone()) {
        current = back;
        steps += 1;
    }
    assert_eq!(steps, CAPACITY);
    assert_eq!(current.selections, vec![(9, 9)]);
}
