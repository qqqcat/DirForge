use proptest::prelude::*;

proptest! {
    #[test]
    fn test_node_store_insert_delete(nodes in prop::collection::vec(arb_node(), 1..100)) {
        let mut store = crate::NodeStore::default();

        // 插入所有节点
        for node in &nodes {
            store.insert(node.clone());
        }

        // 验证数量
        assert_eq!(store.nodes.len(), nodes.len());

        // 删除一半
        for node in nodes.iter().step_by(2) {
            store.remove(node.id);
        }

        // 验证剩余数量
        assert_eq!(store.nodes.len(), (nodes.len() + 1) / 2);
    }

    #[test]
    fn test_string_pool_intern(nodes in prop::collection::vec(".*", 1..50)) {
        let mut store = crate::NodeStore::default();

        // 插入字符串
        let mut ids = Vec::new();
        for s in &nodes {
            let id = store.intern(s);
            ids.push((*s, id));
        }

        // 验证可以解析
        for (s, id) in &ids {
            let resolved = store.resolve_string(*id);
            assert_eq!(resolved, Some(s.as_str()));
        }
    }

    #[test]
    fn test_string_pool_reference_counting(nodes in prop::collection::vec(".*", 1..50)) {
        let mut store = crate::NodeStore::default();

        // 插入相同字符串多次
        let test_str = "test_string";
        let id1 = store.intern(test_str);
        let id2 = store.intern(test_str);

        // 应该返回相同的 ID
        assert_eq!(id1, id2);

        // 验证引用计数
        let rc = store.rc_tracker.get(&id1);
        assert!(rc.is_some());
        assert!(*rc.unwrap() >= 2);
    }
}

/// 生成任意节点
fn arb_node() -> impl Strategy<Value = crate::Node> {
    (
        any::<u64>(),
        any::<u64>(),
        prop_oneof![Just(crate::NodeKind::File), Just(crate::NodeKind::Dir)],
    ).prop_map(|(id, size, kind)| {
        crate::Node {
            id: crate::NodeId(id as usize),
            parent: None,
            name_id: crate::StringId(0),
            path: std::sync::Arc::from(format!("node_{}", id)),
            kind,
            size_self: size,
            size_subtree: size,
            file_count: if matches!(kind, crate::NodeKind::File) { 1 } else { 0 },
            dir_count: if matches!(kind, crate::NodeKind::Dir) { 1 } else { 0 },
            dirty: false,
        }
    })
}
