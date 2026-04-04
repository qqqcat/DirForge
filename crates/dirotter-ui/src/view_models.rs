use super::*;

pub(super) struct InspectorTargetViewModel {
    pub name_value: Arc<str>,
    pub name_hint: &'static str,
    pub path_value: String,
    pub path_hint: &'static str,
    pub size_value: String,
    pub size_hint: String,
}

pub(super) struct DeleteTaskViewModel {
    pub title: &'static str,
    pub description: &'static str,
    pub target_value: String,
    pub target_hint: String,
    pub progress_title: String,
    pub progress_value: String,
    pub progress_hint: String,
    pub elapsed_value: String,
    pub elapsed_hint: &'static str,
    pub current_target_title: Option<String>,
    pub current_target_value: Option<String>,
    pub current_target_hint: Option<&'static str>,
}

pub(super) struct DeleteConfirmViewModel {
    pub intro: &'static str,
    pub target_value: String,
    pub target_hint: &'static str,
    pub size_value: String,
    pub size_hint: String,
    pub recommendation: &'static str,
}

pub(super) struct CleanupDeleteConfirmViewModel {
    pub intro: &'static str,
    pub task_value: String,
    pub task_hint: &'static str,
    pub item_count_value: String,
    pub item_count_hint: &'static str,
    pub estimated_reclaim_value: String,
    pub estimated_reclaim_hint: &'static str,
    pub preview_title: String,
    pub preview_hint: String,
    pub preview_items: Vec<CleanupDeletePreviewItemViewModel>,
    pub confirm_label: &'static str,
}

pub(super) struct CleanupDeletePreviewItemViewModel {
    pub path_value: String,
    pub size_value: String,
}

pub(super) struct InspectorActionsViewModel {
    pub section_description: String,
    pub open_location_label: String,
    pub fast_cleanup_label: String,
    pub recycle_label: String,
    pub permanent_label: String,
    pub release_memory_label: String,
    pub release_memory_tooltip: String,
    pub can_open_location: bool,
    pub can_fast_cleanup: bool,
    pub can_recycle: bool,
    pub can_permanent_delete: bool,
    pub can_release_memory: bool,
    pub info_message: Option<String>,
}

pub(super) struct InspectorFeedbackBannerViewModel {
    pub title: String,
    pub message: String,
}

pub(super) struct InspectorExecutionReportViewModel {
    pub title: String,
    pub summary_value: String,
    pub summary_hint: String,
    pub failure_detail_label: Option<String>,
    pub failure_detail_hint: Option<String>,
}

pub(super) struct ExecutionFailureDetailsViewModel {
    pub title: String,
    pub intro: String,
    pub summary_title: String,
    pub summary_value: String,
    pub summary_hint: String,
    pub close_label: String,
    pub close_hint: String,
    pub items: Vec<ExecutionFailureDetailsItemViewModel>,
}

pub(super) struct ExecutionFailureDetailsItemViewModel {
    pub failure_title: String,
    pub failure_body: String,
    pub path_value: String,
    pub suggestion_title: String,
    pub suggestion_value: String,
    pub technical_detail_title: String,
    pub technical_detail_value: Option<String>,
}

pub(super) struct InspectorWorkspaceContextViewModel {
    pub root_value: String,
    pub root_hint: &'static str,
    pub source_value: String,
    pub source_hint: &'static str,
}

pub(super) struct CleanupDetailsCategoryTabViewModel {
    pub category: CleanupCategory,
    pub label: String,
    pub selected: bool,
}

pub(super) struct CleanupDetailsItemViewModel {
    pub target: SelectedTarget,
    pub checked: bool,
    pub enabled: bool,
    pub selected: bool,
    pub path_value: String,
    pub size_value: String,
    pub risk: RiskLevel,
    pub risk_label: &'static str,
    pub category_label: &'static str,
    pub unused_days_label: Option<String>,
    pub score_label: String,
    pub reason_text: &'static str,
}

pub(super) struct CleanupDetailsWindowViewModel {
    pub review_message: String,
    pub category_tabs: Vec<CleanupDetailsCategoryTabViewModel>,
    pub banner_title: String,
    pub banner_message: String,
    pub selected_count_value: String,
    pub selected_bytes_value: String,
    pub select_safe_enabled: bool,
    pub clear_selected_enabled: bool,
    pub open_selected_enabled: bool,
    pub header_primary_enabled: bool,
    pub permanent_enabled: bool,
    pub footer_primary_enabled: bool,
    pub select_safe_label: String,
    pub clear_selected_label: String,
    pub open_selected_label: String,
    pub header_primary_label: String,
    pub permanent_label: String,
    pub footer_primary_label: String,
    pub close_label: String,
    pub items: Vec<CleanupDetailsItemViewModel>,
}

impl DirOtterNativeApp {
    pub(super) fn materialize_ranked_items(
        paths: &[dirotter_scan::RankedPath],
        limit: usize,
        include_dirs: bool,
    ) -> Vec<dirotter_scan::RankedPath> {
        paths
            .iter()
            .filter(|(path, _)| {
                fs::metadata(path.as_ref())
                    .map(|meta| {
                        if include_dirs {
                            meta.is_dir()
                        } else {
                            meta.is_file()
                        }
                    })
                    .unwrap_or(false)
            })
            .take(limit)
            .map(|(path, size)| (path.clone(), *size))
            .collect()
    }

    pub(super) fn summary_cards(&self) -> Vec<(String, String, String)> {
        let mut cards = vec![
            (
                self.t("文件", "Files").to_string(),
                format_count(self.summary.scanned_files),
                self.t("已发现文件数", "Discovered files").to_string(),
            ),
            (
                self.t("目录", "Directories").to_string(),
                format_count(self.summary.scanned_dirs),
                self.t("已遍历目录数", "Traversed directories").to_string(),
            ),
            (
                self.t("扫描体积", "Scanned Size").to_string(),
                format_bytes(self.summary.bytes_observed),
                self.t(
                    "仅统计已扫描到的文件体积",
                    "Only the file bytes actually scanned",
                )
                .to_string(),
            ),
        ];

        if let Some(volume) = self.current_volume_info() {
            let used = volume.total_bytes.saturating_sub(volume.available_bytes);
            cards.push((
                self.t("磁盘已用", "Volume Used").to_string(),
                format_bytes(used),
                format!(
                    "{} {}  |  {} {}",
                    format_bytes(volume.total_bytes),
                    self.t("总容量", "total"),
                    format_bytes(volume.available_bytes),
                    self.t("可用", "free")
                ),
            ));
        }

        cards.push((
            self.t("错误", "Errors").to_string(),
            format_count(self.summary.error_count),
            self.t("需要关注的问题项", "Items needing attention")
                .to_string(),
        ));

        cards
    }

    pub(super) fn retain_existing_ranked_items(
        items: &[dirotter_scan::RankedPath],
        limit: usize,
        include_dirs: bool,
    ) -> Vec<dirotter_scan::RankedPath> {
        Self::materialize_ranked_items(items, limit, include_dirs)
    }

    pub(super) fn scan_health_summary(&self) -> String {
        let age = self
            .scan_last_event_at
            .map(|instant| instant.elapsed().as_secs_f32())
            .unwrap_or_default();
        format!(
            "{} {:.1}s  |  {} {}  |  {} {}  |  {} {}",
            self.t("最近事件", "Last event"),
            age,
            self.t("丢弃进度", "Dropped progress"),
            format_count(self.scan_dropped_progress),
            self.t("丢弃批次", "Dropped batches"),
            format_count(self.scan_dropped_batches),
            self.t("丢弃快照", "Dropped snapshots"),
            format_count(self.scan_dropped_snapshots),
        )
    }

    pub(super) fn scan_health_short(&self) -> String {
        let age = self
            .scan_last_event_at
            .map(|instant| instant.elapsed().as_secs_f32())
            .unwrap_or_default();
        let path = self
            .scan_current_path
            .as_deref()
            .map(|path| truncate_middle(path, 46))
            .unwrap_or_else(|| self.t("准备中", "Preparing").to_string());
        format!(
            "{} {:.1}s  |  {}",
            self.t("最近事件", "Last event"),
            age,
            path
        )
    }

    pub(super) fn current_ranked_dirs(&self, limit: usize) -> Vec<dirotter_scan::RankedPath> {
        if self.scan_active() && !self.live_top_dirs.is_empty() {
            return Self::retain_existing_ranked_items(&self.live_top_dirs, limit, true);
        }
        if !self.scan_active() && !self.completed_top_dirs.is_empty() {
            return Self::retain_existing_ranked_items(&self.completed_top_dirs, limit, true);
        }

        self.store
            .as_ref()
            .map(|store| {
                store
                    .largest_dirs(limit)
                    .into_iter()
                    .filter(|node| {
                        fs::metadata(store.node_path(node))
                            .map(|meta| meta.is_dir())
                            .unwrap_or(false)
                    })
                    .map(|node| (node.path.clone(), node.size_subtree.max(node.size_self)))
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn current_ranked_files(&self, limit: usize) -> Vec<dirotter_scan::RankedPath> {
        if self.scan_active() && !self.live_top_files.is_empty() {
            return Self::retain_existing_ranked_items(&self.live_top_files, limit, false);
        }
        if !self.scan_active() && !self.completed_top_files.is_empty() {
            return Self::retain_existing_ranked_items(&self.completed_top_files, limit, false);
        }

        self.store
            .as_ref()
            .map(|store| {
                store
                    .top_n_largest_files(limit)
                    .into_iter()
                    .filter(|node| {
                        fs::metadata(store.node_path(node))
                            .map(|meta| meta.is_file())
                            .unwrap_or(false)
                    })
                    .map(|node| (node.path.clone(), node.size_self))
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn ranked_files_in_scope(
        &self,
        scope_path: &str,
        limit: usize,
    ) -> Vec<dirotter_scan::RankedPath> {
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let mut matches: Vec<dirotter_scan::RankedPath> = store
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::File))
            .filter(|node| store.node_path(node) != scope_path)
            .filter(|node| path_within_scope(store.node_path(node), scope_path))
            .map(|node| (node.path.clone(), node.size_self))
            .collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.as_ref().cmp(b.0.as_ref())));
        matches.truncate(limit);
        matches
    }

    pub(super) fn contextual_ranked_files_panel(
        &self,
        limit: usize,
    ) -> (String, String, Vec<dirotter_scan::RankedPath>) {
        if let Some(target) = self.selected_target() {
            let scope_path = match target.kind {
                NodeKind::Dir => Some(target.path.to_string()),
                NodeKind::File => PathBuf::from(target.path.as_ref())
                    .parent()
                    .map(|parent| parent.display().to_string()),
            };

            if let Some(scope_path) = scope_path {
                let scoped_files = self.ranked_files_in_scope(&scope_path, limit);
                if !scoped_files.is_empty() {
                    let scope_name = PathBuf::from(&scope_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| scope_path.clone());
                    return (
                        self.t("所选位置中的最大文件", "Largest Files In Selection")
                            .to_string(),
                        format!(
                            "{}: {}",
                            self.t("当前范围", "Current scope"),
                            truncate_middle(&scope_name, 40)
                        ),
                        scoped_files,
                    );
                }
            }
        }

        (
            self.t("当前最大的文件", "Largest Files Found So Far")
                .to_string(),
            self.t(
                "早期结果可能还不是最终顺序。",
                "Early findings are not yet the final ordering.",
            )
            .to_string(),
            self.current_ranked_files(limit),
        )
    }

    pub(super) fn inspector_target_view_model(
        &self,
        target: &SelectedTarget,
    ) -> InspectorTargetViewModel {
        InspectorTargetViewModel {
            name_value: target.name.clone(),
            name_hint: match target.kind {
                NodeKind::Dir => self.t("目录", "Directory"),
                NodeKind::File => self.t("文件", "File"),
            },
            path_value: truncate_middle(target.path.as_ref(), 34),
            path_hint: self.t("完整路径可在悬浮提示中查看", "Full path available on hover"),
            size_value: format_bytes(target.size_bytes),
            size_hint: format!(
                "{} {} / {} {}",
                format_count(target.file_count),
                self.t("文件", "files"),
                format_count(target.dir_count),
                self.t("目录", "dirs")
            ),
        }
    }

    pub(super) fn delete_task_view_model(&self) -> Option<DeleteTaskViewModel> {
        if let Some(snapshot) = self
            .delete_session
            .as_ref()
            .map(|session| session.snapshot())
        {
            return Some(DeleteTaskViewModel {
                title: match snapshot.mode {
                    ExecutionMode::RecycleBin => {
                        self.t("后台任务：移到回收站", "Background Task: Recycle Bin")
                    }
                    ExecutionMode::FastPurge => {
                        self.t("后台任务：快速清理", "Background Task: Fast Cleanup")
                    }
                    ExecutionMode::Permanent => {
                        self.t("后台任务：永久删除", "Background Task: Permanent Delete")
                    }
                },
                description: self.t(
                    "删除正在后台执行。你可以继续浏览结果，但新的删除操作会暂时锁定。",
                    "Deletion is running in the background. You can keep browsing results, but new delete actions stay locked for now.",
                ),
                target_value: truncate_middle(&snapshot.label, 34),
                target_hint: format!(
                    "{} {}",
                    format_count(snapshot.target_count as u64),
                    self.t("个项目正在执行", "items in flight")
                ),
                progress_title: self.t("进度", "Progress").to_string(),
                progress_value: format!(
                    "{} / {}",
                    format_count(snapshot.completed_count as u64),
                    format_count(snapshot.target_count as u64)
                ),
                progress_hint: format!(
                    "{} {} / {} {}",
                    format_count(snapshot.succeeded_count as u64),
                    self.t("成功", "succeeded"),
                    format_count(snapshot.failed_count as u64),
                    self.t("失败", "failed")
                ),
                elapsed_value: format!("{:.1}s", snapshot.started_at.elapsed().as_secs_f32()),
                elapsed_hint: match snapshot.mode {
                    ExecutionMode::RecycleBin => self.t("回收站删除", "Recycle-bin delete"),
                    ExecutionMode::FastPurge => {
                        self.t("秒移走后后台清除", "Instant move, background purge")
                    }
                    ExecutionMode::Permanent => self.t("永久删除", "Permanent delete"),
                },
                current_target_title: snapshot
                    .current_path
                    .as_ref()
                    .map(|_| self.t("当前项目", "Current Item").to_string()),
                current_target_value: snapshot
                    .current_path
                    .as_deref()
                    .map(|path| truncate_middle(path, 42)),
                current_target_hint: snapshot
                    .current_path
                    .as_ref()
                    .map(|_| self.t("当前处理项目", "Current item")),
            });
        }

        let snapshot = self
            .delete_finalize_session
            .as_ref()
            .and_then(|session| session.snapshot())?;
        Some(DeleteTaskViewModel {
            title: self.t("后台任务：同步结果", "Background Task: Sync Results"),
            description: self.t(
                "删除已完成，结果视图和清理建议正在后台同步。界面会在同步后自动刷新。",
                "Deletion has finished. The result view and cleanup suggestions are synchronizing in the background and will refresh automatically.",
            ),
            target_value: truncate_middle(&snapshot.label, 34),
            target_hint: format!(
                "{} {}",
                format_count(snapshot.target_count as u64),
                self.t("个项目已处理", "items processed")
            ),
            progress_title: self.t("结果同步", "Result Sync").to_string(),
            progress_value: self.t("后台整理中", "Syncing in background").to_string(),
            progress_hint: format!(
                "{} {} / {} {}",
                format_count(snapshot.succeeded_count as u64),
                self.t("成功", "succeeded"),
                format_count(snapshot.failed_count as u64),
                self.t("失败", "failed")
            ),
            elapsed_value: format!("{:.1}s", snapshot.started_at.elapsed().as_secs_f32()),
            elapsed_hint: self.t(
                "删除完成后同步结果视图和清理建议",
                "Synchronizing the result view and cleanup suggestions after deletion",
            ),
            current_target_title: None,
            current_target_value: None,
            current_target_hint: None,
        })
    }

    pub(super) fn delete_confirmation_view_model(
        &self,
        pending: &PendingDeleteConfirmation,
    ) -> Option<DeleteConfirmViewModel> {
        let target = pending.request.targets.first()?;
        Some(DeleteConfirmViewModel {
            intro: self.t(
                "该操作会直接删除文件或目录，不进入回收站。",
                "This action deletes the file or folder directly without using the recycle bin.",
            ),
            target_value: truncate_middle(target.path.as_ref(), 42),
            target_hint: match target.kind {
                NodeKind::Dir => self.t("目录", "Directory"),
                NodeKind::File => self.t("文件", "File"),
            },
            size_value: format_bytes(target.size_bytes),
            size_hint: format!("{:?}", pending.risk),
            recommendation: self.t(
                "建议：如果只是普通清理，优先使用“移到回收站”。永久删除适合明确确认后再执行。",
                "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain.",
            ),
        })
    }

    pub(super) fn cleanup_delete_confirmation_view_model(
        &self,
        request: &CleanupDeleteRequest,
    ) -> CleanupDeleteConfirmViewModel {
        let is_fast_cleanup = request.mode == ExecutionMode::FastPurge;
        let preview_items: Vec<CleanupDeletePreviewItemViewModel> = request
            .targets
            .iter()
            .map(|target| CleanupDeletePreviewItemViewModel {
                path_value: target.path.to_string(),
                size_value: format_bytes(target.size_bytes),
            })
            .collect();
        CleanupDeleteConfirmViewModel {
            intro: self.t(
                if is_fast_cleanup {
                    "将先把建议项快速移出当前目录，再在后台继续释放空间。"
                } else {
                    "将优先把建议项移到回收站，避免直接永久删除。"
                },
                if is_fast_cleanup {
                    "Suggested items will be moved out of the current view first, then reclaimed in the background."
                } else {
                    "Suggested items will be moved to the recycle bin first instead of being deleted permanently."
                },
            ),
            task_value: request.label.clone(),
            task_hint: self.t("规则驱动清理", "Rule-driven cleanup"),
            item_count_value: format_count(request.targets.len() as u64),
            item_count_hint: if is_fast_cleanup {
                self.t("会先进入后台清理区", "Will be staged for background cleanup")
            } else {
                self.t("将进入系统回收站", "Will move to the system recycle bin")
            },
            estimated_reclaim_value: format_bytes(request.estimated_bytes),
            estimated_reclaim_hint: if is_fast_cleanup {
                self.t(
                    "磁盘空间会在后台逐步释放",
                    "Disk space will continue to be reclaimed in the background",
                )
            } else {
                self.t(
                    "实际释放量取决于系统删除结果",
                    "Actual reclaim depends on execution results",
                )
            },
            preview_title: self.t("本次将处理的项目", "Items In This Cleanup").to_string(),
            preview_hint: self
                .t(
                    "下面按完整路径列出本次要处理的全部项目，请滚动确认后再继续。",
                    "The complete target list is shown below. Scroll through the full paths before continuing.",
                )
                .to_string(),
            preview_items,
            confirm_label: if is_fast_cleanup {
                self.t("立即清理", "Clean Now")
            } else {
                self.t("移到回收站", "Move to Recycle Bin")
            },
        }
    }

    fn delete_failure_suggestion(&self, failure_kind: Option<ActionFailureKind>) -> &'static str {
        match failure_kind {
            Some(ActionFailureKind::PermissionDenied) => self.t(
                "权限不足。请检查目标是否为系统目录，或使用更高权限重试。",
                "Permission denied. Check whether the target is protected or retry with higher privileges.",
            ),
            Some(ActionFailureKind::Protected) => self.t(
                "该目标被风险策略拦截，建议优先使用回收站删除或重新评估路径。",
                "This target was blocked by risk protection. Prefer recycle-bin deletion or review the path.",
            ),
            Some(ActionFailureKind::Io) => self.t(
                "文件或目录可能正被占用。关闭相关程序后重试。",
                "The file or directory may be in use. Close related programs and try again.",
            ),
            Some(ActionFailureKind::Missing) => self.t(
                "目标已不存在，界面会在下一次刷新后自动同步。",
                "The target no longer exists. The UI will synchronize on the next refresh.",
            ),
            Some(ActionFailureKind::PlatformUnavailable | ActionFailureKind::NotSupported) => {
                self.t(
                    "当前平台不支持该操作，建议改用回收站删除或系统文件管理器。",
                    "This operation is not supported on the current platform. Try recycle-bin deletion or the system file manager.",
                )
            }
            Some(ActionFailureKind::PrecheckMismatch) => self.t(
                "预检查与执行前状态不一致，建议重新选择该对象后重试。",
                "Precheck no longer matches current state. Re-select the item and try again.",
            ),
            Some(ActionFailureKind::UnsupportedType) => self.t(
                "当前只支持文件和目录，特殊对象请改用系统工具处理。",
                "Only files and directories are supported. Use system tools for special objects.",
            ),
            None => self.t(
                "删除执行失败，请结合失败原因检查路径状态后重试。",
                "Delete action failed. Review the failure reason and retry after checking the target state.",
            ),
        }
    }

    fn delete_failure_title(&self, failure_kind: Option<ActionFailureKind>, retries: u8) -> String {
        match failure_kind {
            Some(ActionFailureKind::PermissionDenied) => {
                self.t("权限不足", "Permission Denied").to_string()
            }
            Some(ActionFailureKind::Protected) => self
                .t("已被风险策略拦截", "Blocked by Safety Policy")
                .to_string(),
            Some(ActionFailureKind::Io) if retries > 0 => self
                .t("重试后仍然失败", "Still Failed After Retries")
                .to_string(),
            Some(ActionFailureKind::Io) => self.t("I/O 执行失败", "I/O Failure").to_string(),
            Some(ActionFailureKind::Missing) => {
                self.t("目标已不存在", "Target Missing").to_string()
            }
            Some(ActionFailureKind::PlatformUnavailable) => {
                self.t("当前平台不可用", "Platform Unavailable").to_string()
            }
            Some(ActionFailureKind::NotSupported) => self
                .t("当前操作不受支持", "Operation Not Supported")
                .to_string(),
            Some(ActionFailureKind::PrecheckMismatch) => self
                .t("执行前状态已变化", "State Changed Before Execution")
                .to_string(),
            Some(ActionFailureKind::UnsupportedType) => self
                .t("对象类型不受支持", "Unsupported Target Type")
                .to_string(),
            None => self.t("删除执行失败", "Delete Failed").to_string(),
        }
    }

    fn delete_failure_body(&self, failure_kind: Option<ActionFailureKind>, retries: u8) -> String {
        match failure_kind {
            Some(ActionFailureKind::PermissionDenied) => self
                .t(
                    "系统拒绝了这次删除请求，通常是因为权限不足或目标受系统保护。",
                    "The system rejected this delete request, usually because of missing privileges or target protection.",
                )
                .to_string(),
            Some(ActionFailureKind::Protected) => self
                .t(
                    "该路径命中了当前风险保护规则，所以这次不会直接执行删除。",
                    "This path matched the current safety rules, so deletion was not executed directly.",
                )
                .to_string(),
            Some(ActionFailureKind::Io) if retries > 0 => format!(
                "{} {} {}。",
                self.t(
                    "系统已经自动重试",
                    "The system already retried this operation",
                ),
                format_count(retries as u64 + 1),
                self.t("次，但仍然没有成功。", "times, but it still did not succeed.")
            ),
            Some(ActionFailureKind::Io) => self
                .t(
                    "执行阶段遇到了 I/O 问题，常见原因是文件占用、临时锁定或权限切换。",
                    "The execution hit an I/O issue, commonly due to file locks, transient handles, or permission transitions.",
                )
                .to_string(),
            Some(ActionFailureKind::Missing) => self
                .t(
                    "在真正执行前，目标已经从磁盘上消失。",
                    "The target disappeared from disk before execution completed.",
                )
                .to_string(),
            Some(ActionFailureKind::PlatformUnavailable | ActionFailureKind::NotSupported) => self
                .t(
                    "当前平台或当前删除方式无法完成这次请求。",
                    "The current platform or delete mode cannot complete this request.",
                )
                .to_string(),
            Some(ActionFailureKind::PrecheckMismatch) => self
                .t(
                    "执行前检查和真实执行时看到的磁盘状态已经不一致。",
                    "The disk state changed between precheck and actual execution.",
                )
                .to_string(),
            Some(ActionFailureKind::UnsupportedType) => self
                .t(
                    "这个对象不是当前删除链路支持的普通文件或目录。",
                    "This object is not a regular file or directory supported by the current delete flow.",
                )
                .to_string(),
            None => self
                .t(
                    "这次删除没有成功完成，请结合下方建议重新检查目标状态。",
                    "This delete did not complete successfully. Review the suggestion below and re-check the target state.",
                )
                .to_string(),
        }
    }

    pub(super) fn inspector_actions_view_model(
        &self,
        selected_target: Option<&SelectedTarget>,
    ) -> InspectorActionsViewModel {
        let has_selection = selected_target.is_some();
        let delete_active = self.delete_active();
        let can_fast_purge_selection = selected_target
            .map(|target| self.can_fast_purge_path(target.path.as_ref()))
            .unwrap_or(false);
        let can_release_memory = !self.system_memory_release_active();
        let info_message = if delete_active {
            Some(
                self.t(
                    "后台删除任务正在执行。你可以继续浏览列表，但新的删除动作会在完成前保持禁用。",
                    "A background delete task is running. You can keep browsing, but new delete actions stay disabled until it finishes.",
                )
                .to_string(),
            )
        } else if self.system_memory_release_active() {
            Some(
                self.t(
                    "系统内存释放正在后台执行。界面不会锁死，完成后会自动显示前后效果。",
                    "System memory release is running in the background. The UI stays responsive and will show the before/after result automatically.",
                )
                .to_string(),
            )
        } else if !has_selection {
            Some(
                self.t(
                    "先从列表、结果视图或其他页面里选中一个文件或文件夹。",
                    "Select a file or folder from a list, result view, or another page first.",
                )
                .to_string(),
            )
        } else {
            None
        };

        InspectorActionsViewModel {
            section_description: self
                .t(
                    "直接在右侧完成清理，不再跳到单独的操作页。",
                    "Delete directly from the inspector instead of jumping to a separate page.",
                )
                .to_string(),
            open_location_label: self.t("打开所在位置", "Open File Location").to_string(),
            fast_cleanup_label: self.t("快速清理缓存", "Fast Cleanup").to_string(),
            recycle_label: self.t("移到回收站", "Move to Recycle Bin").to_string(),
            permanent_label: self.t("永久删除", "Delete Permanently").to_string(),
            release_memory_label: self
                .t("一键释放系统内存", "Release System Memory")
                .to_string(),
            release_memory_tooltip: self
                .t(
                    "基于 Windows 官方能力，尝试收缩当前会话中的高占用进程，并在权限允许时裁剪系统文件缓存。",
                    "Uses Windows-supported memory trimming to shrink large interactive processes and, when allowed, trim the system file cache.",
                )
                .to_string(),
            can_open_location: has_selection,
            can_fast_cleanup: has_selection && can_fast_purge_selection && !delete_active,
            can_recycle: has_selection && !delete_active,
            can_permanent_delete: has_selection && !delete_active,
            can_release_memory,
            info_message,
        }
    }

    pub(super) fn inspector_explorer_feedback_view_model(
        &self,
    ) -> Option<InspectorFeedbackBannerViewModel> {
        let (message, success) = self.explorer_feedback.as_ref()?;
        Some(InspectorFeedbackBannerViewModel {
            title: if *success {
                self.t("已打开所在位置", "Opened Location").to_string()
            } else {
                self.t("打开位置失败", "Open Location Failed").to_string()
            },
            message: message.clone(),
        })
    }

    pub(super) fn inspector_delete_feedback_view_model(
        &self,
    ) -> Option<(InspectorFeedbackBannerViewModel, bool)> {
        let (title, hint, success) = self.delete_feedback_message()?;
        Some((
            InspectorFeedbackBannerViewModel {
                title,
                message: hint,
            },
            success,
        ))
    }

    pub(super) fn inspector_execution_report_view_model(
        &self,
    ) -> Option<InspectorExecutionReportViewModel> {
        let report = self.execution_report.as_ref()?;

        Some(InspectorExecutionReportViewModel {
            title: self.t("最近执行", "Last Action").to_string(),
            summary_value: match report.mode {
                ExecutionMode::RecycleBin => self.t("移到回收站", "Moved to recycle bin"),
                ExecutionMode::FastPurge => self.t("快速清理缓存", "Fast cleanup"),
                ExecutionMode::Permanent => self.t("永久删除", "Permanent delete"),
            }
            .to_string(),
            summary_hint: format!(
                "{} {} / {} {}",
                format_count(report.succeeded as u64),
                self.t("成功", "succeeded"),
                format_count(report.failed as u64),
                self.t("失败", "failed")
            ),
            failure_detail_label: (report.failed > 0).then(|| {
                format!(
                    "{} {}",
                    format_count(report.failed as u64),
                    self.t("失败，查看详情", "failed, view details")
                )
            }),
            failure_detail_hint: (report.failed > 0).then(|| {
                self.t(
                    "打开完整失败列表，查看具体路径、失败原因和处理建议。",
                    "Open the full failed-item list with paths, reasons, and suggestions.",
                )
                .to_string()
            }),
        })
    }

    pub(super) fn execution_failure_details_view_model(
        &self,
    ) -> Option<ExecutionFailureDetailsViewModel> {
        let report = self.execution_report.as_ref()?;
        let items: Vec<ExecutionFailureDetailsItemViewModel> = report
            .items
            .iter()
            .filter(|item| !item.success)
            .map(|item| ExecutionFailureDetailsItemViewModel {
                failure_title: self.delete_failure_title(item.failure_kind, item.retries),
                failure_body: self.delete_failure_body(item.failure_kind, item.retries),
                path_value: item.path.clone(),
                suggestion_title: self.t("建议", "Suggested Next Step").to_string(),
                suggestion_value: self
                    .delete_failure_suggestion(item.failure_kind)
                    .to_string(),
                technical_detail_title: self.t("技术细节", "Technical Detail").to_string(),
                technical_detail_value: (!item.message.is_empty()).then(|| item.message.clone()),
            })
            .collect();
        if items.is_empty() {
            return None;
        }

        Some(ExecutionFailureDetailsViewModel {
            title: self.t("失败详情", "Failure Details").to_string(),
            intro: self
                .t(
                    "以下项目执行失败。这里会显示完整路径、失败原因和对应建议。",
                    "These items failed to execute. Full paths, failure reasons, and suggestions are listed here.",
                )
                .to_string(),
            summary_title: self.t("执行方式", "Execution").to_string(),
            summary_value: match report.mode {
                ExecutionMode::RecycleBin => self.t("移到回收站", "Moved to recycle bin"),
                ExecutionMode::FastPurge => self.t("快速清理缓存", "Fast cleanup"),
                ExecutionMode::Permanent => self.t("永久删除", "Permanent delete"),
            }
            .to_string(),
            summary_hint: format!(
                "{} {} / {} {}",
                format_count(report.succeeded as u64),
                self.t("成功", "succeeded"),
                format_count(report.failed as u64),
                self.t("失败", "failed")
            ),
            close_label: self.t("关闭", "Close").to_string(),
            close_hint: self
                .t(
                    "关闭详情并返回右侧摘要。",
                    "Close the details and return to the inspector summary.",
                )
                .to_string(),
            items,
        })
    }

    pub(super) fn inspector_workspace_context_view_model(
        &self,
    ) -> InspectorWorkspaceContextViewModel {
        InspectorWorkspaceContextViewModel {
            root_value: truncate_middle(&self.root_input, 32),
            root_hint: self.t("当前扫描目标", "Current scan target"),
            source_value: self
                .selection
                .source
                .map(|source| self.source_label(source))
                .unwrap_or_else(|| self.t("无", "None"))
                .to_string(),
            source_hint: self.t("当前聚焦来源", "Selection source"),
        }
    }

    pub(super) fn cleanup_details_window_view_model(
        &self,
        category: CleanupCategory,
        items: &[CleanupCandidate],
    ) -> CleanupDetailsWindowViewModel {
        let categories = self
            .cleanup
            .analysis
            .as_ref()
            .map(|analysis| analysis.categories.clone())
            .unwrap_or_default();
        let (selected_count, selected_bytes) = self.selected_cleanup_totals(category);
        let delete_active = self.delete_active();
        let header_primary_label = if category == CleanupCategory::Cache {
            self.t("快速清理选中缓存", "Fast Cleanup Selected")
        } else {
            self.t("移到回收站", "Move to Recycle Bin")
        };
        let footer_primary_label = if category == CleanupCategory::Cache {
            self.t("快速清理选中缓存", "Fast Cleanup Selected")
        } else {
            self.t("清理选中项", "Clean Selected")
        };

        CleanupDetailsWindowViewModel {
            review_message: self
                .t(
                    "按分类检查后再决定清理范围。",
                    "Review by category before deciding what to clean.",
                )
                .to_string(),
            category_tabs: categories
                .into_iter()
                .map(|entry| CleanupDetailsCategoryTabViewModel {
                    category: entry.category,
                    label: format!(
                        "{}  {}",
                        self.cleanup_category_label(entry.category),
                        format_bytes(entry.total_bytes)
                    ),
                    selected: self.cleanup.detail_category == Some(entry.category),
                })
                .collect(),
            banner_title: self.cleanup_category_label(category).to_string(),
            banner_message: self
                .t(
                    "绿色会默认勾选，黄色默认不勾选；红色项请点击条目后用“打开所选位置”自行确认处理。",
                    "Safe items are selected by default and warning items stay unchecked. For red items, click the row and use Open Selected Location for manual review.",
                )
                .to_string(),
            selected_count_value: format_count(selected_count as u64),
            selected_bytes_value: format_bytes(selected_bytes),
            select_safe_enabled: !delete_active,
            clear_selected_enabled: !delete_active,
            open_selected_enabled: self.selected_target().is_some(),
            header_primary_enabled: selected_count > 0 && !delete_active,
            permanent_enabled: selected_count > 0 && !delete_active,
            footer_primary_enabled: selected_count > 0 && !delete_active,
            select_safe_label: self.t("全选安全项", "Select Safe").to_string(),
            clear_selected_label: self.t("清空所选", "Clear Selected").to_string(),
            open_selected_label: self.t("打开所选位置", "Open Selected").to_string(),
            header_primary_label: header_primary_label.to_string(),
            permanent_label: self.t("永久删除", "Delete Permanently").to_string(),
            footer_primary_label: footer_primary_label.to_string(),
            close_label: self.t("关闭", "Close").to_string(),
            items: items
                .iter()
                .map(|item| CleanupDetailsItemViewModel {
                    target: item.target.clone(),
                    checked: self.cleanup.selected_paths.contains(item.target.path.as_ref()),
                    enabled: item.risk != RiskLevel::High,
                    selected: self.selection_matches_target(&item.target),
                    path_value: truncate_middle(item.target.path.as_ref(), 72),
                    size_value: format_bytes(item.target.size_bytes),
                    risk: item.risk,
                    risk_label: self.cleanup_risk_label(item.risk),
                    category_label: self.cleanup_category_label(item.category),
                    unused_days_label: item.unused_days.map(|unused_days| {
                        format!("{} {}", unused_days, self.t("天未使用", "days unused"))
                    }),
                    score_label: format!("{} {:.1}", self.t("评分", "Score"), item.cleanup_score),
                    reason_text: self.cleanup_reason_text(item),
                })
                .collect(),
        }
    }
}
