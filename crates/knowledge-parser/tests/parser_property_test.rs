use proptest::prelude::*;
use knowledge_parser::scope_stack::{ScopeStack, ScopeType};

proptest! {
    #[test]
    fn proptest_scope_stack_push_pop_consistency(
        ops in prop::collection::vec(
            prop_oneof![
                Just("push_global"),
                Just("push_function"),
                Just("push_block"),
                Just("push_class"),
                Just("push_module"),
                Just("pop"),
            ],
            0..100,
        ),
    ) {
        let mut stack = ScopeStack::new();
        let mut expected_depth: usize = 0;

        for op in &ops {
            match *op {
                "push_global" => {
                    stack.push_scope(ScopeType::Global, 0);
                    expected_depth += 1;
                }
                "push_function" => {
                    stack.push_scope(ScopeType::Function, 1);
                    expected_depth += 1;
                }
                "push_block" => {
                    stack.push_scope(ScopeType::Block, 2);
                    expected_depth += 1;
                }
                "push_class" => {
                    stack.push_scope(ScopeType::Class, 3);
                    expected_depth += 1;
                }
                "push_module" => {
                    stack.push_scope(ScopeType::Module, 4);
                    expected_depth += 1;
                }
                "pop" => {
                    let frame = stack.pop_scope();
                    if expected_depth > 0 {
                        prop_assert!(frame.is_some());
                        expected_depth -= 1;
                    } else {
                        prop_assert!(frame.is_none());
                    }
                }
                _ => {}
            }
            prop_assert_eq!(stack.depth(), expected_depth);
        }

        while expected_depth > 0 {
            let frame = stack.pop_scope();
            prop_assert!(frame.is_some());
            expected_depth -= 1;
            prop_assert_eq!(stack.depth(), expected_depth);
        }

        let frame = stack.pop_scope();
        prop_assert!(frame.is_none());
    }
}

proptest! {
    #[test]
    fn proptest_scope_stack_define_lookup_consistency(
        symbols in prop::collection::vec("[a-zA-Z_][a-zA-Z0-9_]{0,20}", 0..50),
    ) {
        let mut stack = ScopeStack::new();
        stack.push_scope(ScopeType::Global, 0);

        for sym in &symbols {
            stack.define_symbol(sym);
        }

        for sym in &symbols {
            prop_assert!(
                stack.lookup_symbol(sym).is_some(),
                "已定义的符号 '{}' 应能被查找到",
                sym
            );
        }

        prop_assert!(
            stack.lookup_symbol("___nonexistent_symbol___").is_none(),
            "未定义的符号不应被查找到"
        );
    }
}

proptest! {
    #[test]
    fn proptest_scope_stack_shadowing_inner_priority(
        outer_sym in "[a-zA-Z_][a-zA-Z0-9_]{0,10}",
        inner_sym in "[a-zA-Z_][a-zA-Z0-9_]{0,10}",
    ) {
        let mut stack = ScopeStack::new();
        stack.push_scope(ScopeType::Global, 0);
        stack.define_symbol(&outer_sym);

        stack.push_scope(ScopeType::Function, 10);
        stack.define_symbol(&inner_sym);

        let (level, _) = stack.lookup_symbol(&inner_sym).unwrap();
        prop_assert_eq!(level, 0, "内层符号应优先于外层");

        let (outer_level, _) = stack.lookup_symbol(&outer_sym).unwrap();
        if outer_sym == inner_sym {
            prop_assert_eq!(outer_level, 0, "同名符号应返回内层定义");
        } else {
            prop_assert_eq!(outer_level, 1, "不同名的外层符号应在 level 1");
        }
    }
}

proptest! {
    #[test]
    fn proptest_chunker_no_panic_on_any_input(input in "\\PC*") {
        use knowledge_parser::text_splitter::TextSplitterBlocker;

        let blocker = TextSplitterBlocker::new();
        let result = blocker.split_to_blocks(&input);
        prop_assert!(result.is_ok(), "分块器不应 panic，输入: {:?}", input);
    }
}
