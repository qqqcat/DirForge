use crate::Lang;

#[path = "i18n_generated_group0.rs"]
mod generated_group0;
#[path = "i18n_generated_group1.rs"]
mod generated_group1;
#[path = "i18n_generated_group2.rs"]
mod generated_group2;
#[path = "i18n_generated_group3a.rs"]
mod generated_group3a;
#[path = "i18n_generated_group3b.rs"]
mod generated_group3b;
#[path = "i18n_generated.rs"]
mod legacy_generated;
#[path = "i18n_missing_keys_patch.rs"]
mod missing_keys_patch;

pub(crate) fn parse_lang_setting(value: &str) -> Option<Lang> {
    match value {
        "ar" => Some(Lang::Ar),
        "de" => Some(Lang::De),
        "en" => Some(Lang::En),
        "he" => Some(Lang::He),
        "hi" => Some(Lang::Hi),
        "id" => Some(Lang::Id),
        "it" => Some(Lang::It),
        "ja" => Some(Lang::Ja),
        "ko" => Some(Lang::Ko),
        "nl" => Some(Lang::Nl),
        "pl" => Some(Lang::Pl),
        "ru" => Some(Lang::Ru),
        "zh" => Some(Lang::Zh),
        "fr" => Some(Lang::Fr),
        "es" => Some(Lang::Es),
        "th" => Some(Lang::Th),
        "tr" => Some(Lang::Tr),
        "uk" => Some(Lang::Uk),
        "vi" => Some(Lang::Vi),
        _ => None,
    }
}

pub(crate) fn lang_setting_value(lang: Lang) -> &'static str {
    match lang {
        Lang::Ar => "ar",
        Lang::De => "de",
        Lang::En => "en",
        Lang::He => "he",
        Lang::Hi => "hi",
        Lang::Id => "id",
        Lang::It => "it",
        Lang::Ja => "ja",
        Lang::Ko => "ko",
        Lang::Nl => "nl",
        Lang::Pl => "pl",
        Lang::Ru => "ru",
        Lang::Zh => "zh",
        Lang::Fr => "fr",
        Lang::Es => "es",
        Lang::Th => "th",
        Lang::Tr => "tr",
        Lang::Uk => "uk",
        Lang::Vi => "vi",
    }
}

pub(crate) fn lang_native_label(lang: Lang) -> &'static str {
    match lang {
        Lang::Ar => "العربية",
        Lang::De => "Deutsch",
        Lang::En => "English",
        Lang::He => "עברית",
        Lang::Hi => "हिन्दी",
        Lang::Id => "Bahasa Indonesia",
        Lang::It => "Italiano",
        Lang::Ja => "日本語",
        Lang::Ko => "한국어",
        Lang::Nl => "Nederlands",
        Lang::Pl => "Polski",
        Lang::Ru => "Русский",
        Lang::Zh => "中文",
        Lang::Fr => "Français",
        Lang::Es => "Español",
        Lang::Th => "ไทย",
        Lang::Tr => "Türkçe",
        Lang::Uk => "Українська",
        Lang::Vi => "Tiếng Việt",
    }
}

pub(crate) fn lang_picker_label(lang: Lang) -> &'static str {
    match lang {
        Lang::Ar => "العربية · Arabic",
        Lang::De => "Deutsch · German",
        Lang::En => "English",
        Lang::He => "עברית · Hebrew",
        Lang::Hi => "हिन्दी · Hindi",
        Lang::Id => "Bahasa Indonesia · Indonesian",
        Lang::It => "Italiano · Italian",
        Lang::Ja => "日本語 · Japanese",
        Lang::Ko => "한국어 · Korean",
        Lang::Nl => "Nederlands · Dutch",
        Lang::Pl => "Polski · Polish",
        Lang::Ru => "Русский · Russian",
        Lang::Zh => "中文 · Chinese",
        Lang::Fr => "Français · French",
        Lang::Es => "Español · Spanish",
        Lang::Th => "ไทย · Thai",
        Lang::Tr => "Türkçe · Turkish",
        Lang::Uk => "Українська · Ukrainian",
        Lang::Vi => "Tiếng Việt · Vietnamese",
    }
}

pub(crate) fn supported_languages() -> &'static [Lang] {
    &[
        Lang::Zh,
        Lang::En,
        Lang::Ar,
        Lang::Nl,
        Lang::Fr,
        Lang::De,
        Lang::He,
        Lang::Hi,
        Lang::Id,
        Lang::It,
        Lang::Ja,
        Lang::Ko,
        Lang::Pl,
        Lang::Ru,
        Lang::Es,
        Lang::Th,
        Lang::Tr,
        Lang::Uk,
        Lang::Vi,
    ]
}

pub(crate) fn detect_lang_from_locale(locale: &str) -> Lang {
    let locale = locale.trim().to_lowercase();
    if locale.starts_with("ar") {
        Lang::Ar
    } else if locale.starts_with("de") {
        Lang::De
    } else if locale.starts_with("zh") {
        Lang::Zh
    } else if locale.starts_with("fr") {
        Lang::Fr
    } else if locale.starts_with("es") {
        Lang::Es
    } else if locale.starts_with("he") || locale.starts_with("iw") {
        Lang::He
    } else if locale.starts_with("hi") {
        Lang::Hi
    } else if locale.starts_with("id") || locale.starts_with("in") {
        Lang::Id
    } else if locale.starts_with("it") {
        Lang::It
    } else if locale.starts_with("ja") {
        Lang::Ja
    } else if locale.starts_with("ko") {
        Lang::Ko
    } else if locale.starts_with("nl") {
        Lang::Nl
    } else if locale.starts_with("pl") {
        Lang::Pl
    } else if locale.starts_with("ru") {
        Lang::Ru
    } else if locale.starts_with("th") {
        Lang::Th
    } else if locale.starts_with("tr") {
        Lang::Tr
    } else if locale.starts_with("uk") {
        Lang::Uk
    } else if locale.starts_with("vi") {
        Lang::Vi
    } else {
        Lang::En
    }
}

pub(crate) fn translate_ui<'a>(lang: Lang, zh: &'a str, en: &'a str) -> &'a str {
    if let Some(translated) = missing_keys_patch::translate_missing_ui_key(lang, en) {
        return translated;
    }
    match lang {
        Lang::Ar => {
            let translated = generated_group1::translate_ar(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_ar(en)
            }
        }
        Lang::De => {
            let translated = generated_group1::translate_de(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_de(en)
            }
        }
        Lang::En => en,
        Lang::He => {
            let translated = generated_group1::translate_he(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_he(en)
            }
        }
        Lang::Hi => {
            let translated = generated_group1::translate_hi(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_hi(en)
            }
        }
        Lang::Id => {
            let translated = generated_group1::translate_id(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_id(en)
            }
        }
        Lang::It => {
            let translated = generated_group2::translate_it(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_it(en)
            }
        }
        Lang::Ja => {
            let translated = generated_group2::translate_ja(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_ja(en)
            }
        }
        Lang::Ko => {
            let translated = generated_group2::translate_ko(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_ko(en)
            }
        }
        Lang::Nl => {
            let translated = generated_group2::translate_nl(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_nl(en)
            }
        }
        Lang::Pl => {
            let translated = generated_group2::translate_pl(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_pl(en)
            }
        }
        Lang::Ru => {
            let translated = generated_group3a::translate_ru(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_ru(en)
            }
        }
        Lang::Zh => zh,
        Lang::Fr => translate_fr(en),
        Lang::Es => translate_es(en),
        Lang::Th => {
            let translated = generated_group3a::translate_th(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_th(en)
            }
        }
        Lang::Tr => {
            let translated = generated_group3a::translate_tr(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_tr(en)
            }
        }
        Lang::Uk => {
            let translated = generated_group3b::translate_uk(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_uk(en)
            }
        }
        Lang::Vi => {
            let translated = generated_group3b::translate_vi(en);
            if translated != en {
                translated
            } else {
                legacy_generated::translate_vi(en)
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn has_translation(lang: Lang, en: &str) -> bool {
    if missing_keys_patch::has_missing_ui_translation(lang, en) {
        return true;
    }
    match lang {
        Lang::En | Lang::Zh => true,
        Lang::Fr => has_translation_fr(en),
        Lang::Es => has_translation_es(en),
        Lang::Ar => {
            generated_group1::has_translation_ar(en) || legacy_generated::has_translation_ar(en)
        }
        Lang::De => {
            generated_group1::has_translation_de(en) || legacy_generated::has_translation_de(en)
        }
        Lang::He => {
            generated_group1::has_translation_he(en) || legacy_generated::has_translation_he(en)
        }
        Lang::Hi => {
            generated_group1::has_translation_hi(en) || legacy_generated::has_translation_hi(en)
        }
        Lang::Id => {
            generated_group1::has_translation_id(en) || legacy_generated::has_translation_id(en)
        }
        Lang::It => {
            generated_group2::has_translation_it(en) || legacy_generated::has_translation_it(en)
        }
        Lang::Ja => {
            generated_group2::has_translation_ja(en) || legacy_generated::has_translation_ja(en)
        }
        Lang::Ko => {
            generated_group2::has_translation_ko(en) || legacy_generated::has_translation_ko(en)
        }
        Lang::Nl => {
            generated_group2::has_translation_nl(en) || legacy_generated::has_translation_nl(en)
        }
        Lang::Pl => {
            generated_group2::has_translation_pl(en) || legacy_generated::has_translation_pl(en)
        }
        Lang::Ru => {
            generated_group3a::has_translation_ru(en) || legacy_generated::has_translation_ru(en)
        }
        Lang::Th => {
            generated_group3a::has_translation_th(en) || legacy_generated::has_translation_th(en)
        }
        Lang::Tr => {
            generated_group3a::has_translation_tr(en) || legacy_generated::has_translation_tr(en)
        }
        Lang::Uk => {
            generated_group3b::has_translation_uk(en) || legacy_generated::has_translation_uk(en)
        }
        Lang::Vi => {
            generated_group3b::has_translation_vi(en) || legacy_generated::has_translation_vi(en)
        }
    }
}

macro_rules! translation_table {
    ($lookup:ident, { $($en:literal => $translated:literal,)* }) => {
        fn $lookup(en: &str) -> Option<&'static str> {
            match en {
                $($en => Some($translated),)*
                _ => None,
            }
        }
    };
}

translation_table!(lookup_fr, {
    "DirOtter Workspace" => "Espace de travail DirOtter",
    "Quick Scan (Recommended)" => "Analyse rapide (recommandée)",
    "Deep Scan" => "Analyse approfondie",
    "Large Disk Mode" => "Mode grand disque",
    "Reach actionable results faster. Best for routine cleanup and most local disks." => "Atteignez plus vite des résultats exploitables. Idéal pour le nettoyage courant et la plupart des disques locaux.",
    "Use a steadier cadence for complex directory trees and first-pass investigations." => "Adopte un rythme plus stable pour les arborescences complexes et les premières inspections.",
    "Reduce UI refresh pressure for very large drives, external disks, or extremely dense folders." => "Réduit la pression de rafraîchissement de l'interface pour les très grands disques, les disques externes ou les dossiers très denses.",
    "All modes scan the same scope. The only difference is pacing and UI update cadence." => "Tous les modes analysent le même périmètre. Seuls le rythme et la cadence de mise à jour de l'interface changent.",
    "Idle" => "Au repos",
    "Scanning" => "Analyse en cours",
    "Completed" => "Terminé",
    "Deleting" => "Suppression en cours",
    "Delete executed" => "Suppression exécutée",
    "Delete failed" => "Échec de suppression",
    "Cancelled" => "Annulé",
    "Finalizing" => "Finalisation",
    "Files" => "Fichiers",
    "Discovered files" => "Fichiers détectés",
    "Directories" => "Répertoires",
    "Traversed directories" => "Répertoires parcourus",
    "Scanned Size" => "Taille analysée",
    "Only the file bytes actually scanned" => "Uniquement les octets de fichiers réellement analysés",
    "Volume Used" => "Espace utilisé",
    "total" => "total",
    "free" => "libre",
    "Errors" => "Erreurs",
    "Items needing attention" => "Éléments à vérifier",
    "Cache" => "Cache",
    "Downloads" => "Téléchargements",
    "Videos" => "Vidéos",
    "Archives" => "Archives",
    "Installers" => "Installateurs",
    "Images" => "Images",
    "System" => "Système",
    "Other" => "Autres",
    "Safe" => "Sûr",
    "Warning" => "Attention",
    "Manual Review" => "Révision manuelle",
    "Matched AppData / Temp / Cache path rules." => "Correspond aux règles de chemin AppData / Temp / Cache.",
    "Located under Downloads and usually needs human review." => "Situé dans Téléchargements et nécessite généralement une vérification humaine.",
    "Large video file." => "Fichier vidéo volumineux.",
    "Archive package." => "Archive compressée.",
    "Installer package." => "Paquet d'installation.",
    "Image asset." => "Ressource image.",
    "System path or system-managed file. Open its location and review manually." => "Chemin système ou fichier géré par le système. Ouvrez son emplacement puis vérifiez-le manuellement.",
    "Large unclassified file." => "Grand fichier non classé.",
    "Clean Selected" => "Nettoyer la sélection",
    "Quick Cache Cleanup" => "Nettoyage rapide du cache",
    "Moved to Recycle Bin" => "Déplacé vers la corbeille",
    "items were moved to the system recycle bin and can be restored there." => "éléments ont été déplacés vers la corbeille système et peuvent y être restaurés.",
    "Deleted Permanently" => "Supprimé définitivement",
    "items were permanently deleted and cannot be undone in the current build." => "éléments ont été supprimés définitivement et ne peuvent pas être restaurés dans cette version.",
    "Cleanup Partially Completed" => "Nettoyage partiellement terminé",
    "succeeded" => "réussis",
    "failed" => "échoués",
    "Permission denied. Check whether the target is protected or retry with higher privileges." => "Permission refusée. Vérifiez si la cible est protégée ou relancez avec des privilèges plus élevés.",
    "This target was blocked by risk protection. Prefer recycle-bin deletion or review the path." => "Cette cible a été bloquée par la protection de risque. Préférez la corbeille ou revoyez le chemin.",
    "The file or directory may be in use. Close related programs and try again." => "Le fichier ou le répertoire est peut-être en cours d'utilisation. Fermez les programmes concernés puis réessayez.",
    "The target no longer exists. The UI will synchronize on the next refresh." => "La cible n'existe plus. L'interface se resynchronisera au prochain rafraîchissement.",
    "This operation is not supported on the current platform. Try recycle-bin deletion or the system file manager." => "Cette opération n'est pas prise en charge sur la plateforme actuelle. Essayez la corbeille ou le gestionnaire de fichiers système.",
    "Precheck no longer matches current state. Re-select the item and try again." => "La pré-vérification ne correspond plus à l'état actuel. Reselectonnez l'élément puis réessayez.",
    "Only files and directories are supported. Use system tools for special objects." => "Seuls les fichiers et répertoires sont pris en charge. Utilisez les outils système pour les objets spéciaux.",
    "Delete action failed. Review the message below and try again." => "La suppression a échoué. Consultez le message ci-dessous puis réessayez.",
    "Delete Failed" => "Échec de suppression",
    "Table" => "Tableau",
    "Result View" => "Vue des résultats",
    "History" => "Historique",
    "Error" => "Erreur",
    "Last event" => "Dernier événement",
    "Dropped progress" => "Progressions ignorées",
    "Dropped batches" => "Lots ignorés",
    "Dropped snapshots" => "Instantanés ignorés",
    "Preparing" => "Préparation",
    "Largest Files In Selection" => "Plus gros fichiers de la sélection",
    "Current scope" => "Périmètre actuel",
    "Largest Files Found So Far" => "Plus gros fichiers trouvés jusqu'ici",
    "Early findings are not yet the final ordering." => "Les premiers résultats ne reflètent pas encore l'ordre final.",
    "Storage Intelligence" => "Intelligence de stockage",
    "A calmer way to understand your file tree." => "Une manière plus sereine de comprendre votre arborescence.",
    "Navigation" => "Navigation",
    "Drive Overview" => "Vue du disque",
    "Like mainstream disk analyzers: start with volume space, then inspect the largest folders and files." => "Comme les analyseurs de disque classiques : commencez par l'espace du volume, puis inspectez les plus gros dossiers et fichiers.",
    "Pick a drive to begin scanning." => "Choisissez un lecteur pour commencer l'analyse.",
    "Start from a quick-drive button, or refine the path before scanning." => "Commencez avec un bouton de lecteur rapide, ou ajustez le chemin avant l'analyse.",
    "Ready for a New Pass" => "Prêt pour une nouvelle passe",
    "When the scan completes, this page will surface volume usage, largest folders, and largest files first." => "Une fois l'analyse terminée, cette page mettra d'abord en avant l'espace utilisé, les plus gros dossiers et les plus gros fichiers.",
    "Preparing scan path..." => "Préparation du chemin d'analyse...",
    "Scan Still Running" => "Analyse toujours en cours",
    "Currently working on:" => "Traitement en cours :",
    "Largest Folders" => "Plus gros dossiers",
    "Largest Files" => "Plus gros fichiers",
    "Cleanup Suggestions" => "Suggestions de nettoyage",
    "DirOtter screens safe items first, then separates anything that still needs your review." => "DirOtter filtre d'abord les éléments sûrs, puis isole ce qui demande encore votre vérification.",
    "You Can Reclaim" => "Vous pouvez libérer",
    "Only counts items that pass the rule-based suggestion filter" => "Ne compte que les éléments qui passent le filtre de suggestions basé sur des règles",
    "Only safe cache items" => "Uniquement les éléments de cache sûrs",
    "No Suggestions Yet" => "Aucune suggestion pour l'instant",
    "The current scan does not yet contain strong cleanup suggestions. You can still start from the largest files and folders." => "L'analyse actuelle ne contient pas encore de suggestions de nettoyage fortes. Vous pouvez toujours commencer par les plus gros fichiers et dossiers.",
    "blocked" => "bloqué",
    "View Details" => "Voir le détail",
    "Scan Target" => "Cible d'analyse",
    "Set the scan scope first, then choose a scan mode that matches the job." => "Définissez d'abord le périmètre d'analyse, puis choisissez un mode adapté à la tâche.",
    "Root path" => "Chemin racine",
    "Quick Drives" => "Lecteurs rapides",
    "No mounted volumes were detected. You can still enter any path manually." => "Aucun volume monté n'a été détecté. Vous pouvez toujours saisir un chemin manuellement.",
    "Used" => "Utilisé",
    "Total" => "Total",
    "Click a drive button to scan it immediately, or type any custom path in the field above." => "Cliquez sur un lecteur pour l'analyser immédiatement, ou saisissez un chemin personnalisé dans le champ ci-dessus.",
    "Scan Mode" => "Mode d'analyse",
    "DirOtter now handles batch and snapshot pacing automatically. You no longer need to tune technical knobs." => "DirOtter gère maintenant automatiquement le rythme des lots et des instantanés. Vous n'avez plus besoin de régler des paramètres techniques.",
    "Start Scan" => "Démarrer l'analyse",
    "Scan Setup" => "Paramètres d'analyse",
    "Use the top-right stop button while a scan is running." => "Utilisez le bouton d'arrêt en haut à droite pendant une analyse.",
    "Volume Summary" => "Résumé du volume",
    "Use the volume-level summary to orient yourself before drilling into directories." => "Utilisez le résumé du volume pour vous orienter avant d'explorer les répertoires.",
    "Free" => "Libre",
    "System volume info" => "Infos du volume système",
    "Scanned" => "Analysé",
    "Total file bytes scanned so far" => "Total des octets de fichiers analysés jusqu'ici",
    "Unreadable or skipped paths" => "Chemins illisibles ou ignorés",
    "Scan Coverage" => "Couverture d'analyse",
    "Files counted" => "Fichiers comptabilisés",
    "Folders" => "Dossiers",
    "Folders traversed" => "Dossiers parcourus",
    "Live Scan" => "Analyse en direct",
    "This page shows the largest items discovered so far, not the final result. Internal performance counters have been moved to Diagnostics." => "Cette page montre les plus gros éléments détectés jusqu'ici, pas le résultat final. Les compteurs internes ont été déplacés vers Diagnostic.",
    "This Is a Live Incremental View" => "Vue incrémentale en direct",
    "Results keep updating while the scan runs. Use Overview after completion for the final summary. Working on:" => "Les résultats continuent de se mettre à jour pendant l'analyse. Utilisez Vue d'ensemble après la fin pour le résumé final. En cours :",
    "Recently Scanned Files" => "Fichiers analysés récemment",
    "rows" => "lignes",
    "This page only shows completed scan results. It is not bound to live scanning. Inspect one directory level at a time and drill in only when needed." => "Cette page n'affiche que les résultats terminés. Elle n'est pas liée à l'analyse en direct. Inspectez un seul niveau de répertoire à la fois et approfondissez seulement si nécessaire.",
    "Treemap Stays Out of Live Updates" => "La vue des résultats reste hors des mises à jour en direct",
    "The result view is generated only after scan completion, avoiding UI thread churn, huge node counts, and layout overhead piling up together." => "La vue des résultats n'est générée qu'après la fin de l'analyse, évitant les recharges du thread UI, les très grands nombres de nœuds et le coût cumulé de mise en page.",
    "No Completed Result Yet" => "Aucun résultat final pour l'instant",
    "Complete a scan first, or wait until a cached snapshot is loaded before using this result view." => "Terminez d'abord une analyse, ou attendez le chargement d'un instantané en cache avant d'utiliser cette vue.",
    "Lightweight Result View" => "Vue légère des résultats",
    "Current directory:" => "Répertoire actuel :",
    "Only direct children are shown. No whole-tree recursion and no live layout work." => "Seuls les enfants directs sont affichés. Pas de récursion sur tout l'arbre ni de mise en page en direct.",
    "Up One Level" => "Monter d'un niveau",
    "Back to Root" => "Retour à la racine",
    "Use Selected Directory" => "Utiliser le répertoire sélectionné",
    "Current Level Size" => "Taille du niveau actuel",
    "Used as the local baseline" => "Utilisé comme référence locale",
    "Direct Children" => "Enfants directs",
    "Current directory only" => "Répertoire actuel seulement",
    "Display Cap" => "Limite d'affichage",
    "Keeps large folders responsive" => "Maintient la réactivité des grands dossiers",
    "No Children to Show at This Level" => "Aucun enfant à afficher à ce niveau",
    "This directory may be empty, or the cached result does not currently have child nodes for it." => "Ce répertoire est peut-être vide, ou le résultat en cache n'a pas encore de nœuds enfants pour lui.",
    "Directory Result Bars" => "Barres de résultat du répertoire",
    "Click an item to sync Inspector. Directories can drill into the next level." => "Cliquez sur un élément pour synchroniser l'inspecteur. Les répertoires peuvent ouvrir le niveau suivant.",
    "DIR" => "RÉP",
    "FILE" => "FICHIER",
    "files" => "fichiers",
    "subdirs" => "sous-dossiers",
    "File item" => "Élément fichier",
    "Open Level" => "Ouvrir le niveau",
    "Review previous scans with human-friendly formatting and clearer snapshot summaries." => "Revisitez les analyses précédentes avec un format plus lisible et des résumés d'instantanés plus clairs.",
    "Refresh" => "Rafraîchir",
    "Snapshot Detail" => "Détail de l'instantané",
    "ID" => "ID",
    "File count" => "Nombre de fichiers",
    "Dirs" => "Doss.",
    "Directory count" => "Nombre de répertoires",
    "Bytes" => "Octets",
    "Historical scanned file size" => "Taille historique des fichiers analysés",
    "dirs" => "doss.",
    "scanned" => "analysé",
    "Keep error categories and jump actions while reducing raw-text noise." => "Conserve les catégories d'erreur et les actions de saut tout en réduisant le bruit du texte brut.",
    "User" => "Utilisateur",
    "Input or permission issues" => "Problèmes de saisie ou de permission",
    "Transient" => "Transitoire",
    "Retryable transient failures" => "Échecs transitoires réessayables",
    "System-level failures" => "Défaillances système",
    "All" => "Tous",
    "Filter" => "Filtre",
    "Inspect" => "Inspecter",
    "Diagnostics" => "Diagnostic",
    "Keep the structured JSON, but surface export actions and explanation more clearly." => "Conservez le JSON structuré, mais rendez les actions d'export et leur explication plus visibles.",
    "Refresh diagnostics" => "Rafraîchir le diagnostic",
    "Export diagnostics bundle" => "Exporter le paquet de diagnostic",
    "Save Current Snapshot" => "Enregistrer l'instantané actuel",
    "Record Scan Summary" => "Enregistrer le résumé du scan",
    "Export Errors CSV" => "Exporter le CSV des erreurs",
    "Settings" => "Paramètres",
    "Keep DirOtter calm, low-contrast, and comfortable for long sessions." => "Gardez DirOtter calme, peu contrasté et confortable pour les longues sessions.",
    "A Comfort-First Workspace" => "Un espace de travail centré sur le confort",
    "Language, theme, and font fallback all apply immediately. The goal here is not flashy UI, but a steadier workspace for long file-tree sessions." => "La langue, le thème et les polices de secours s'appliquent immédiatement. L'objectif n'est pas une interface tape-à-l'œil, mais un espace de travail plus stable pour de longues sessions sur l'arborescence.",
    "Interface Language" => "Langue de l'interface",
    "Manual selection overrides automatic locale detection." => "La sélection manuelle remplace la détection automatique de la langue système.",
    "Interface Theme" => "Thème de l'interface",
    "Dark is better for long analysis sessions; light stays restrained and low contrast." => "Le mode sombre convient mieux aux longues analyses ; le mode clair reste sobre et peu contrasté.",
    "Light" => "Clair",
    "Dark" => "Sombre",
    "Advanced Tools" => "Outils avancés",
    "Keeps history, errors, and diagnostics behind a secondary entry. Most cleanup flows do not need them by default." => "Place l'historique, les erreurs et le diagnostic derrière une entrée secondaire. La plupart des parcours de nettoyage n'en ont pas besoin par défaut.",
    "Enabled" => "Activé",
    "Hidden" => "Masqué",
    "DirOtter is not a loud cleaner UI. It is a quieter, analytical workspace with restrained emphasis." => "DirOtter n'est pas une interface de nettoyage tapageuse. C'est un espace analytique plus calme, avec des accents maîtrisés.",
    "Current Mode" => "Mode actuel",
    "Localization Notes" => "Notes de localisation",
    "The app now prefers CJK-capable system fallback fonts (Windows prioritizes Microsoft YaHei / DengXian) so Chinese labels do not render as tofu boxes." => "L'application privilégie maintenant des polices système de secours compatibles CJK (Windows priorise Microsoft YaHei / DengXian) afin d'éviter les caractères remplacés pour le chinois.",
    "The first launch can still infer language from the system locale, but the manual choice here overrides auto-detection." => "Le premier lancement peut encore déduire la langue à partir du système, mais le choix manuel ici remplace cette détection.",
    "Why DirOtter" => "Pourquoi DirOtter",
    "Dir points to directories, while Otter brings a clever, tidy, exploratory character. The product should feel like a calm storage analyzer, not a noisy junk cleaner." => "Dir renvoie aux répertoires, tandis que Otter apporte une image astucieuse, ordonnée et exploratoire. Le produit doit ressembler à un analyseur de stockage calme, pas à un nettoyeur bruyant.",
    "Stop Scan" => "Arrêter l'analyse",
    "Cancel" => "Annuler",
    "Inspector" => "Inspecteur",
    "Details for the current selection" => "Détails de la sélection actuelle",
    "Name" => "Nom",
    "Directory" => "Répertoire",
    "File" => "Fichier",
    "Path" => "Chemin",
    "Full path available on hover" => "Chemin complet visible au survol",
    "Size" => "Taille",
    "No file or folder selected yet. Pick one from the live list, history, errors, or treemap." => "Aucun fichier ou dossier n'est encore sélectionné. Choisissez-en un dans la liste en direct, l'historique, les erreurs ou la vue des résultats.",
    "No file or folder is selected yet. Pick one from the live list, result view, or another page." => "Aucun fichier ni dossier n'est encore sélectionné. Choisissez-en un depuis la liste en direct, la vue des résultats ou une autre page.",
    "Background Task: Recycle Bin" => "Tâche en arrière-plan : corbeille",
    "Background Task: Permanent Delete" => "Tâche en arrière-plan : suppression définitive",
    "Deletion is running in the background. You can keep browsing results, but new delete actions stay locked for now." => "La suppression s'exécute en arrière-plan. Vous pouvez continuer à parcourir les résultats, mais les nouvelles suppressions restent verrouillées pour l'instant.",
    "Target" => "Cible",
    "items in flight" => "éléments en cours",
    "Elapsed" => "Temps écoulé",
    "Recycle-bin delete" => "Suppression vers la corbeille",
    "Permanent delete" => "Suppression définitive",
    "Quick Actions" => "Actions rapides",
    "Delete directly from the inspector instead of jumping to a separate page." => "Supprimez directement depuis l'inspecteur au lieu d'ouvrir une page séparée.",
    "Release Memory" => "Libérer la mémoire",
    "Release System Memory" => "Libérer la mémoire système",
    "Clear transient app caches and try to shrink the current process. Disabled during scan or delete work." => "Efface les caches temporaires de l'application et tente de réduire le processus actuel. Désactivé pendant une analyse ou une suppression.",
    "Uses Windows-supported memory trimming to shrink large interactive processes and, when allowed, trim the system file cache." => "Utilise les mécanismes pris en charge par Windows pour réduire les processus interactifs les plus gourmands et, si les droits le permettent, rogner le cache fichiers système.",
    "Memory release stays disabled while scan or delete tasks are active so the current work is not interrupted." => "La libération de mémoire reste désactivée pendant les analyses ou suppressions afin de ne pas interrompre le travail en cours.",
    "System memory release is running in the background. The UI stays responsive and will show the before/after result automatically." => "La libération de mémoire système s’exécute en arrière-plan. L’interface reste réactive et affichera automatiquement le résultat avant/après.",
    "Open File Location" => "Ouvrir l'emplacement",
    "Opened the target location in the system file manager." => "L'emplacement cible a été ouvert dans le gestionnaire de fichiers système.",
    "Failed to open location" => "Échec d'ouverture de l'emplacement",
    "Move to Recycle Bin" => "Déplacer vers la corbeille",
    "Delete Permanently" => "Supprimer définitivement",
    "A background delete task is running. You can keep browsing, but new delete actions stay disabled until it finishes." => "Une suppression en arrière-plan est en cours. Vous pouvez continuer à parcourir les résultats, mais les nouvelles suppressions restent désactivées jusqu'à la fin.",
    "Select a file or folder from a list, treemap, history, or errors first." => "Sélectionnez d'abord un fichier ou un dossier depuis une liste, la vue des résultats, l'historique ou les erreurs.",
    "Select a file or folder from a list, result view, or another page first." => "Sélectionnez d'abord un fichier ou un dossier depuis une liste, la vue des résultats ou une autre page.",
    "Opened Location" => "Emplacement ouvert",
    "Open Location Failed" => "Échec de l'ouverture de l'emplacement",
    "Last Action" => "Dernière action",
    "Moved to recycle bin" => "Déplacé vers la corbeille",
    "Result" => "Résultat",
    "Failure" => "Échec",
    "Workspace Context" => "Contexte de l'espace de travail",
    "Root" => "Racine",
    "Current scan target" => "Cible d'analyse actuelle",
    "Source" => "Source",
    "None" => "Aucune",
    "Selection source" => "Source de sélection",
    "Confirm Permanent Delete" => "Confirmer la suppression définitive",
    "This action deletes the file or folder directly without using the recycle bin." => "Cette action supprime directement le fichier ou le dossier sans utiliser la corbeille.",
    "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain." => "Recommandation : privilégiez la corbeille pour le nettoyage courant. Utilisez la suppression définitive uniquement si vous êtes certain.",
    "Cleanup Details" => "Détails du nettoyage",
    "Safe items are selected by default and warning items stay unchecked. For red items, click the row and use Open Selected Location for manual review." => "Les éléments sûrs sont sélectionnés par défaut et les avertissements restent décochés. Pour les éléments rouges, cliquez sur la ligne puis utilisez Ouvrir l'emplacement sélectionné pour vérifier manuellement.",
    "days unused" => "jours sans usage",
    "Score" => "Score",
    "Close" => "Fermer",
    "Confirm Cleanup" => "Confirmer le nettoyage",
    "Suggested items will be moved to the recycle bin first instead of being deleted permanently." => "Les éléments suggérés seront d'abord déplacés vers la corbeille au lieu d'être supprimés définitivement.",
    "Task" => "Tâche",
    "Rule-driven cleanup" => "Nettoyage piloté par règles",
    "Items" => "Éléments",
    "Will be moved to recycle bin" => "Sera déplacé vers la corbeille",
    "Estimated Reclaim" => "Récupération estimée",
    "Actual reclaim depends on execution results" => "La récupération réelle dépend du résultat de l'exécution",
    "more items not shown" => "autres éléments non affichés",
    "Start Cleanup" => "Démarrer le nettoyage",
    "errors" => "erreurs",
    "used" => "utilisé",
    "items" => "éléments",
    "Moving to Recycle Bin in Background" => "Déplacement vers la corbeille en arrière-plan",
    "Deleting Permanently in Background" => "Suppression définitive en arrière-plan",
    "You can keep browsing scan results. The UI will synchronize automatically when deletion finishes." => "Vous pouvez continuer à parcourir les résultats d'analyse. L'interface se synchronisera automatiquement lorsque la suppression sera terminée.",
    "System is processing the delete request" => "Le système traite la demande de suppression",
    "Finalizing final results..." => "Finalisation des résultats...",
    "Finalizing Final Results" => "Finalisation des résultats",
    "Manual path (optional)" => "Chemin manuel (optionnel)",
    "Start with a drive button first. Only type a manual path when you need a subfolder." => "Commencez d'abord par un bouton de lecteur. Saisissez un chemin manuel uniquement si vous avez besoin d'un sous-dossier.",
    "Scanning finds storage hotspots. Use the separate memory action in Quick Actions for memory release." => "L'analyse repère les points chauds de stockage. Utilisez l'action mémoire séparée dans Actions rapides pour libérer la mémoire.",
    "There is no scan result to save yet." => "Aucun résultat de scan n'est encore disponible à enregistrer.",
    "Saved the current snapshot manually." => "L'instantané actuel a été enregistré manuellement.",
    "Failed to save snapshot" => "Échec de l'enregistrement de l'instantané",
    "Recorded the current scan summary manually." => "Le résumé du scan actuel a été enregistré manuellement.",
    "Failed to record scan history" => "Échec de l'enregistrement de l'historique du scan",
    "Exported the errors CSV." => "Le CSV des erreurs a été exporté.",
    "Failed to export errors CSV" => "Échec de l'export du CSV des erreurs",
    "Exported the diagnostics bundle." => "Le paquet de diagnostic a été exporté.",
    "One-Tap Boost" => "Accélération en un clic",
    "Boost Now (Recommended)" => "Accélérer maintenant (recommandé)",
    "Start Boost Scan" => "Lancer l’analyse d’accélération",
    "Review Boost Suggestions" => "Voir les suggestions d’accélération",
    "Review Largest Items" => "Voir les plus gros éléments",
    "Advanced Maintenance" => "Maintenance avancée",
    "The safest and most direct one-tap boost right now is cache cleanup, with about" => "L’action d’accélération en un clic la plus sûre et la plus directe pour le moment est le nettoyage du cache, avec environ",
    "Run a scan first so DirOtter can identify safe cache and the largest cleanup targets." => "Lancez d’abord une analyse afin que DirOtter puisse identifier les caches sûrs et les plus grosses cibles de nettoyage.",
    "Potential system-slowing storage targets were found, but they still need your confirmation before execution." => "Des éléments de stockage susceptibles de ralentir le système ont été trouvés, mais ils nécessitent encore votre confirmation avant exécution.",
    "No safe one-tap boost stands out right now. Starting from the largest folders and files is usually the most effective next step." => "Aucune accélération sûre en un clic ne se démarque pour le moment. Commencer par les plus grands dossiers et fichiers est généralement l’étape suivante la plus efficace.",
    "There is no obvious one-tap boost action right now. Start from the largest folders and files below." => "Aucune action évidente d’accélération en un clic n’est disponible pour le moment. Commencez par les plus grands dossiers et fichiers ci-dessous.",
    "These actions are mainly for diagnostics and recovery. They are not part of the normal one-tap speed path for everyday users." => "Ces actions servent surtout au diagnostic et à la récupération. Elles ne font pas partie du parcours normal d’accélération en un clic pour les utilisateurs quotidiens.",
    "Clean Interrupted Cleanup Area" => "Nettoyer la zone de suppression interrompue",
    "Optimize DirOtter Memory" => "Optimiser la mémoire de DirOtter",
    "Clean Up Staging" => "Nettoyer la zone staging",
    "Maintenance Done" => "Maintenance terminée",
    "Maintenance Failed" => "Échec de maintenance",
    "Working set reclaimed about" => "Mémoire de travail récupérée d’environ",
    "System free memory increased by about" => "La mémoire libre du système a augmenté d’environ",
    "Trimmed processes" => "Processus réduits",
    "Scanned candidates" => "Candidats analysés",
    "System file cache trimmed" => "Cache fichiers système rogné",
    "System memory release failed" => "La libération de mémoire système a échoué",
    "A disk snapshot was saved first, so the result can be reloaded later." => "Un instantané disque a d’abord été enregistré, afin que le résultat puisse être rechargé plus tard.",
    "Cleared the current result and optimized DirOtter memory usage." => "Le résultat actuel a été vidé et l’empreinte mémoire de DirOtter a été optimisée.",
    "Cleared current results, but Windows working-set trimming failed" => "Les résultats actuels ont été vidés, mais la réduction du working set Windows a échoué",
    "system free" => "mémoire système libre",
    "load" => "charge",
    "Cleared the current result and requested DirOtter memory trimming." => "Le résultat actuel a été vidé et une réduction de la mémoire de DirOtter a été demandée.",
    "Cleared current results, but memory trimming failed" => "Les résultats actuels ont été vidés, mais la réduction mémoire a échoué",
    "Manually cleaned remaining staging items." => "Les éléments restants en staging ont été nettoyés manuellement.",
    "Failed to clean staging" => "Échec du nettoyage de la zone staging",
    "Cleaned leftover items from the interrupted cleanup area." => "Les éléments restants de la zone de suppression interrompue ont été nettoyés.",
    "Failed to clean the interrupted cleanup area" => "Échec du nettoyage de la zone de suppression interrompue",
    "Wait for scan or delete tasks to finish before releasing memory." => "Attendez la fin des tâches d'analyse ou de suppression avant de libérer la mémoire.",
    "Memory release completed: transient caches were cleared and the current process was trimmed." => "Libération de mémoire terminée : les caches temporaires ont été vidés et le processus actuel a été réduit.",
    "There is no additional application-side memory to release right now." => "Il n'y a pas de mémoire supplémentaire côté application à libérer pour le moment.",
    "Complete a scan first before using this result view. DirOtter does not auto-load old cached results here, so the UI stays responsive." => "Terminez d'abord une analyse avant d'utiliser cette vue des résultats. DirOtter ne recharge pas automatiquement ici les anciens résultats en cache afin de préserver la réactivité de l'interface.",
    "Failure Details" => "Détails des échecs",
    "These items failed to execute. Full paths, failure reasons, and suggestions are listed here." => "Ces éléments n'ont pas pu être exécutés. Les chemins complets, les causes d'échec et les suggestions sont listés ici.",
    "Close the details and return to the inspector summary." => "Fermez ce panneau et revenez au résumé de l'inspecteur.",
    "Progress" => "Progression",
    "Current Item" => "Élément en cours",
    "Current item" => "Élément en cours",
    "Items In This Cleanup" => "Éléments inclus dans ce nettoyage",
    "Delete action failed. Review the failure reason and retry after checking the target state." => "La suppression a échoué. Consultez la raison de l'échec puis réessayez après avoir vérifié l'état de la cible.",
    "Background Task: Fast Cleanup" => "Tâche en arrière-plan : nettoyage rapide",
    "Instant move, background purge" => "Déplacement immédiat, purge en arrière-plan",
    "Will be staged for background cleanup" => "Sera placé dans la zone de nettoyage en arrière-plan",
    "Will move to the system recycle bin" => "Sera déplacé vers la corbeille du système",
    "Disk space will continue to be reclaimed in the background" => "L'espace disque continuera d'être récupéré en arrière-plan",
    "Clean Now" => "Nettoyer maintenant",
    "Fast Cleanup" => "Nettoyage rapide",
    "Fast cleanup" => "Nettoyage rapide",
    "Select Safe" => "Sélectionner les éléments sûrs",
    "Clear Selected" => "Effacer la sélection",
    "Open Selected" => "Ouvrir la sélection",
    "Background Task: Sync Results" => "Tâche en arrière-plan : synchronisation des résultats",
    "Deletion has finished. The result view and cleanup suggestions are synchronizing in the background and will refresh automatically." => "La suppression est terminée. La vue des résultats et les suggestions de nettoyage se synchronisent en arrière-plan et se mettront à jour automatiquement.",
    "items processed" => "éléments traités",
    "Result Sync" => "Synchronisation des résultats",
    "Syncing in background" => "Synchronisation en arrière-plan",
    "Synchronizing the result view and cleanup suggestions after deletion" => "Synchronisation de la vue des résultats et des suggestions de nettoyage après la suppression",
    "Synchronizing Cleanup Results" => "Synchronisation des résultats du nettoyage",
    "Deletion finished. The result view and cleanup suggestions are being synchronized in the background." => "La suppression est terminée. La vue des résultats et les suggestions de nettoyage sont en cours de synchronisation en arrière-plan.",
    "System is synchronizing post-delete results" => "Le système synchronise les résultats après suppression",
    "Result View Is Waiting For Cleanup Sync" => "La vue des résultats attend la fin de la synchronisation du nettoyage",
    "Background deletion or result synchronization is still running. DirOtter will resume the result view after it finishes so snapshot loading and result rebuilding do not block the UI thread." => "La suppression en arrière-plan ou la synchronisation des résultats est toujours en cours. DirOtter réouvrira la vue des résultats une fois terminée afin que le chargement du snapshot et la reconstruction des résultats ne bloquent pas le thread UI.",
    "Loading Saved Result Snapshot" => "Chargement du snapshot de résultat enregistré",
    "DirOtter is loading the saved result snapshot in the background. The lightweight result view will open automatically when it is ready, without decompressing or rebuilding the whole result tree on the current UI frame." => "DirOtter charge le snapshot de résultat enregistré en arrière-plan. La vue légère des résultats s'ouvrira automatiquement lorsqu'elle sera prête, sans décompresser ni reconstruire tout l'arbre de résultats sur la frame UI courante.",
    "Permission Denied" => "Permission refusée",
    "Open the full failed-item list with paths, reasons, and suggestions." => "Ouvrez la liste complète des éléments en échec avec les chemins, raisons et suggestions.",
    "Execution" => "Exécution",
    "Still Failed After Retries" => "Échec après plusieurs tentatives",
    "Blocked by Safety Policy" => "Bloqué par la politique de sécurité",
    "I/O Failure" => "Échec d'E/S",
    "Target Missing" => "Cible introuvable",
    "Platform Unavailable" => "Plateforme indisponible",
    "Operation Not Supported" => "Opération non prise en charge",
    "State Changed Before Execution" => "L'état a changé avant l'exécution",
    "Unsupported Target Type" => "Type de cible non pris en charge",
    "The system rejected this delete request, usually because of missing privileges or target protection." => "Le système a rejeté cette demande de suppression, généralement à cause de privilèges insuffisants ou d'une protection de la cible.",
    "This path matched the current safety rules, so deletion was not executed directly." => "Ce chemin correspond aux règles de sécurité actuelles, la suppression directe n'a donc pas été exécutée.",
    "The system already retried this operation" => "Le système a déjà retenté cette opération",
    "times, but it still did not succeed." => "fois, mais elle a quand même échoué.",
    "The execution hit an I/O issue, commonly due to file locks, transient handles, or permission transitions." => "L'exécution a rencontré un problème d'E/S, souvent causé par un fichier verrouillé, un handle transitoire ou un changement de permissions.",
    "The target disappeared from disk before execution completed." => "La cible a disparu du disque avant la fin de l'exécution.",
    "The current platform or delete mode cannot complete this request." => "La plateforme actuelle ou le mode de suppression choisi ne permet pas de terminer cette demande.",
    "The disk state changed between precheck and actual execution." => "L'état du disque a changé entre la pré-vérification et l'exécution réelle.",
    "This object is not a regular file or directory supported by the current delete flow." => "Cet objet n'est ni un fichier ni un dossier standard pris en charge par le flux de suppression actuel.",
    "This delete did not complete successfully. Review the suggestion below and re-check the target state." => "Cette suppression ne s'est pas terminée correctement. Consultez la suggestion ci-dessous puis revérifiez l'état de la cible.",
    "failed, view details" => "échecs, voir le détail",
    "Suggested Next Step" => "Étape recommandée",
    "Technical Detail" => "Détail technique",
});

translation_table!(lookup_es, {
    "DirOtter Workspace" => "Espacio de trabajo DirOtter",
    "Quick Scan (Recommended)" => "Escaneo rápido (recomendado)",
    "Deep Scan" => "Escaneo profundo",
    "Large Disk Mode" => "Modo de disco grande",
    "Reach actionable results faster. Best for routine cleanup and most local disks." => "Llega más rápido a resultados accionables. Ideal para limpieza rutinaria y la mayoría de discos locales.",
    "Use a steadier cadence for complex directory trees and first-pass investigations." => "Usa un ritmo más estable para árboles de directorios complejos y revisiones iniciales.",
    "Reduce UI refresh pressure for very large drives, external disks, or extremely dense folders." => "Reduce la presión de refresco de la interfaz en discos muy grandes, discos externos o carpetas extremadamente densas.",
    "All modes scan the same scope. The only difference is pacing and UI update cadence." => "Todos los modos escanean el mismo alcance. La única diferencia es el ritmo y la cadencia de actualización de la interfaz.",
    "Idle" => "Inactivo",
    "Scanning" => "Escaneando",
    "Completed" => "Completado",
    "Deleting" => "Eliminando",
    "Delete executed" => "Eliminación ejecutada",
    "Delete failed" => "Falló la eliminación",
    "Cancelled" => "Cancelado",
    "Finalizing" => "Finalizando",
    "Files" => "Archivos",
    "Discovered files" => "Archivos detectados",
    "Directories" => "Directorios",
    "Traversed directories" => "Directorios recorridos",
    "Scanned Size" => "Tamaño escaneado",
    "Only the file bytes actually scanned" => "Solo los bytes de archivos realmente escaneados",
    "Volume Used" => "Espacio usado",
    "total" => "total",
    "free" => "libre",
    "Errors" => "Errores",
    "Items needing attention" => "Elementos que requieren atención",
    "Cache" => "Caché",
    "Downloads" => "Descargas",
    "Videos" => "Vídeos",
    "Archives" => "Archivos comprimidos",
    "Installers" => "Instaladores",
    "Images" => "Imágenes",
    "System" => "Sistema",
    "Other" => "Otros",
    "Safe" => "Seguro",
    "Warning" => "Advertencia",
    "Manual Review" => "Revisión manual",
    "Matched AppData / Temp / Cache path rules." => "Coincide con las reglas de ruta AppData / Temp / Cache.",
    "Located under Downloads and usually needs human review." => "Está en Descargas y normalmente requiere revisión humana.",
    "Large video file." => "Archivo de vídeo grande.",
    "Archive package." => "Paquete comprimido.",
    "Installer package." => "Paquete instalador.",
    "Image asset." => "Recurso de imagen.",
    "System path or system-managed file. Open its location and review manually." => "Ruta del sistema o archivo gestionado por el sistema. Abre su ubicación y revísalo manualmente.",
    "Large unclassified file." => "Archivo grande sin clasificar.",
    "Clean Selected" => "Limpiar selección",
    "Quick Cache Cleanup" => "Limpieza rápida de caché",
    "Moved to Recycle Bin" => "Movido a la papelera",
    "items were moved to the system recycle bin and can be restored there." => "elementos se movieron a la papelera del sistema y pueden restaurarse allí.",
    "Deleted Permanently" => "Eliminado permanentemente",
    "items were permanently deleted and cannot be undone in the current build." => "elementos se eliminaron permanentemente y no pueden deshacerse en esta versión.",
    "Cleanup Partially Completed" => "Limpieza completada parcialmente",
    "succeeded" => "correctos",
    "failed" => "fallidos",
    "Permission denied. Check whether the target is protected or retry with higher privileges." => "Permiso denegado. Comprueba si el destino está protegido o vuelve a intentarlo con más privilegios.",
    "This target was blocked by risk protection. Prefer recycle-bin deletion or review the path." => "Este destino fue bloqueado por la protección de riesgo. Prefiere moverlo a la papelera o revisar la ruta.",
    "The file or directory may be in use. Close related programs and try again." => "Es posible que el archivo o directorio esté en uso. Cierra los programas relacionados y vuelve a intentarlo.",
    "The target no longer exists. The UI will synchronize on the next refresh." => "El destino ya no existe. La interfaz se sincronizará en la próxima actualización.",
    "This operation is not supported on the current platform. Try recycle-bin deletion or the system file manager." => "Esta operación no es compatible con la plataforma actual. Prueba con la papelera o con el explorador del sistema.",
    "Precheck no longer matches current state. Re-select the item and try again." => "La verificación previa ya no coincide con el estado actual. Vuelve a seleccionar el elemento e inténtalo otra vez.",
    "Only files and directories are supported. Use system tools for special objects." => "Solo se admiten archivos y directorios. Usa herramientas del sistema para objetos especiales.",
    "Delete action failed. Review the message below and try again." => "La acción de eliminación falló. Revisa el mensaje de abajo y vuelve a intentarlo.",
    "Delete Failed" => "Fallo de eliminación",
    "Table" => "Tabla",
    "Result View" => "Vista de resultados",
    "History" => "Historial",
    "Error" => "Error",
    "Last event" => "Último evento",
    "Dropped progress" => "Progreso descartado",
    "Dropped batches" => "Lotes descartados",
    "Dropped snapshots" => "Instantáneas descartadas",
    "Preparing" => "Preparando",
    "Largest Files In Selection" => "Archivos más grandes de la selección",
    "Current scope" => "Alcance actual",
    "Largest Files Found So Far" => "Archivos más grandes encontrados hasta ahora",
    "Early findings are not yet the final ordering." => "Los hallazgos iniciales todavía no reflejan el orden final.",
    "Storage Intelligence" => "Inteligencia de almacenamiento",
    "A calmer way to understand your file tree." => "Una forma más serena de entender tu árbol de archivos.",
    "Navigation" => "Navegación",
    "Drive Overview" => "Resumen de unidad",
    "Like mainstream disk analyzers: start with volume space, then inspect the largest folders and files." => "Como los analizadores de disco convencionales: empieza por el espacio del volumen y luego inspecciona las carpetas y archivos más grandes.",
    "Pick a drive to begin scanning." => "Elige una unidad para comenzar el escaneo.",
    "Start from a quick-drive button, or refine the path before scanning." => "Empieza con un botón de unidad rápida o ajusta la ruta antes de escanear.",
    "Ready for a New Pass" => "Listo para una nueva pasada",
    "When the scan completes, this page will surface volume usage, largest folders, and largest files first." => "Cuando termine el escaneo, esta página mostrará primero el uso del volumen, las carpetas más grandes y los archivos más grandes.",
    "Preparing scan path..." => "Preparando ruta de escaneo...",
    "Scan Still Running" => "El escaneo sigue en curso",
    "Currently working on:" => "Trabajando ahora en:",
    "Largest Folders" => "Carpetas más grandes",
    "Largest Files" => "Archivos más grandes",
    "Cleanup Suggestions" => "Sugerencias de limpieza",
    "DirOtter screens safe items first, then separates anything that still needs your review." => "DirOtter filtra primero los elementos seguros y separa lo que todavía necesita tu revisión.",
    "You Can Reclaim" => "Puedes recuperar",
    "Only counts items that pass the rule-based suggestion filter" => "Solo cuenta los elementos que pasan el filtro de sugerencias basado en reglas",
    "Only safe cache items" => "Solo elementos seguros de caché",
    "No Suggestions Yet" => "Todavía no hay sugerencias",
    "The current scan does not yet contain strong cleanup suggestions. You can still start from the largest files and folders." => "El escaneo actual todavía no contiene sugerencias de limpieza sólidas. Aún puedes empezar por los archivos y carpetas más grandes.",
    "blocked" => "bloqueado",
    "View Details" => "Ver detalles",
    "Scan Target" => "Destino de escaneo",
    "Set the scan scope first, then choose a scan mode that matches the job." => "Define primero el alcance del escaneo y luego elige un modo acorde a la tarea.",
    "Root path" => "Ruta raíz",
    "Quick Drives" => "Unidades rápidas",
    "No mounted volumes were detected. You can still enter any path manually." => "No se detectaron volúmenes montados. Aún puedes introducir cualquier ruta manualmente.",
    "Used" => "Usado",
    "Total" => "Total",
    "Click a drive button to scan it immediately, or type any custom path in the field above." => "Haz clic en una unidad para escanearla de inmediato o escribe una ruta personalizada en el campo superior.",
    "Scan Mode" => "Modo de escaneo",
    "DirOtter now handles batch and snapshot pacing automatically. You no longer need to tune technical knobs." => "DirOtter ahora gestiona automáticamente el ritmo de lotes e instantáneas. Ya no necesitas ajustar controles técnicos.",
    "Start Scan" => "Iniciar escaneo",
    "Scan Setup" => "Configuración de escaneo",
    "Use the top-right stop button while a scan is running." => "Usa el botón de detener arriba a la derecha mientras el escaneo esté en curso.",
    "Volume Summary" => "Resumen del volumen",
    "Use the volume-level summary to orient yourself before drilling into directories." => "Usa el resumen del volumen para orientarte antes de profundizar en directorios.",
    "Free" => "Libre",
    "System volume info" => "Información del volumen del sistema",
    "Scanned" => "Escaneado",
    "Total file bytes scanned so far" => "Total de bytes de archivo escaneados hasta ahora",
    "Unreadable or skipped paths" => "Rutas ilegibles u omitidas",
    "Scan Coverage" => "Cobertura del escaneo",
    "Files counted" => "Archivos contados",
    "Folders" => "Carpetas",
    "Folders traversed" => "Carpetas recorridas",
    "Live Scan" => "Escaneo en vivo",
    "This page shows the largest items discovered so far, not the final result. Internal performance counters have been moved to Diagnostics." => "Esta página muestra los elementos más grandes encontrados hasta ahora, no el resultado final. Los contadores internos se movieron a Diagnóstico.",
    "This Is a Live Incremental View" => "Vista incremental en vivo",
    "Results keep updating while the scan runs. Use Overview after completion for the final summary. Working on:" => "Los resultados siguen actualizándose mientras el escaneo avanza. Usa Resumen al terminar para ver el cierre final. Trabajando en:",
    "Recently Scanned Files" => "Archivos escaneados recientemente",
    "rows" => "filas",
    "This page only shows completed scan results. It is not bound to live scanning. Inspect one directory level at a time and drill in only when needed." => "Esta página solo muestra resultados completados. No está vinculada al escaneo en vivo. Inspecciona un nivel de directorio cada vez y profundiza solo cuando haga falta.",
    "Treemap Stays Out of Live Updates" => "La vista de resultados queda fuera de las actualizaciones en vivo",
    "The result view is generated only after scan completion, avoiding UI thread churn, huge node counts, and layout overhead piling up together." => "La vista de resultados solo se genera al terminar el escaneo, evitando carga del hilo UI, grandes conteos de nodos y sobrecoste de maquetación acumulado.",
    "No Completed Result Yet" => "Todavía no hay un resultado final",
    "Complete a scan first, or wait until a cached snapshot is loaded before using this result view." => "Completa primero un escaneo o espera a que se cargue una instantánea en caché antes de usar esta vista.",
    "Lightweight Result View" => "Vista ligera de resultados",
    "Current directory:" => "Directorio actual:",
    "Only direct children are shown. No whole-tree recursion and no live layout work." => "Solo se muestran hijos directos. Sin recursión de todo el árbol ni maquetación en vivo.",
    "Up One Level" => "Subir un nivel",
    "Back to Root" => "Volver a la raíz",
    "Use Selected Directory" => "Usar directorio seleccionado",
    "Current Level Size" => "Tamaño del nivel actual",
    "Used as the local baseline" => "Usado como referencia local",
    "Direct Children" => "Hijos directos",
    "Current directory only" => "Solo el directorio actual",
    "Display Cap" => "Límite de visualización",
    "Keeps large folders responsive" => "Mantiene responsivas las carpetas grandes",
    "No Children to Show at This Level" => "No hay hijos que mostrar en este nivel",
    "This directory may be empty, or the cached result does not currently have child nodes for it." => "Puede que este directorio esté vacío o que el resultado en caché no tenga nodos hijo para él en este momento.",
    "Directory Result Bars" => "Barras de resultados del directorio",
    "Click an item to sync Inspector. Directories can drill into the next level." => "Haz clic en un elemento para sincronizar el inspector. Los directorios pueden abrir el siguiente nivel.",
    "DIR" => "DIR",
    "FILE" => "ARCHIVO",
    "files" => "archivos",
    "subdirs" => "subdirectorios",
    "File item" => "Elemento de archivo",
    "Open Level" => "Abrir nivel",
    "Review previous scans with human-friendly formatting and clearer snapshot summaries." => "Revisa escaneos anteriores con formato más legible y resúmenes de instantáneas más claros.",
    "Refresh" => "Actualizar",
    "Snapshot Detail" => "Detalle de instantánea",
    "ID" => "ID",
    "File count" => "Cantidad de archivos",
    "Dirs" => "Dirs",
    "Directory count" => "Cantidad de directorios",
    "Bytes" => "Bytes",
    "Historical scanned file size" => "Tamaño histórico de archivos escaneados",
    "dirs" => "dirs",
    "scanned" => "escaneado",
    "Keep error categories and jump actions while reducing raw-text noise." => "Mantiene categorías de error y acciones de salto reduciendo el ruido del texto bruto.",
    "User" => "Usuario",
    "Input or permission issues" => "Problemas de entrada o permisos",
    "Transient" => "Transitorio",
    "Retryable transient failures" => "Fallos transitorios reintentables",
    "System-level failures" => "Fallos a nivel de sistema",
    "All" => "Todos",
    "Filter" => "Filtro",
    "Inspect" => "Inspeccionar",
    "Diagnostics" => "Diagnóstico",
    "Keep the structured JSON, but surface export actions and explanation more clearly." => "Mantiene el JSON estructurado, pero hace más visibles las acciones de exportación y su explicación.",
    "Refresh diagnostics" => "Actualizar diagnóstico",
    "Export diagnostics bundle" => "Exportar paquete de diagnóstico",
    "Save Current Snapshot" => "Guardar instantánea actual",
    "Record Scan Summary" => "Registrar resumen del escaneo",
    "Export Errors CSV" => "Exportar CSV de errores",
    "Settings" => "Configuración",
    "Keep DirOtter calm, low-contrast, and comfortable for long sessions." => "Mantiene DirOtter sereno, con bajo contraste y cómodo para sesiones largas.",
    "A Comfort-First Workspace" => "Un espacio de trabajo centrado en la comodidad",
    "Language, theme, and font fallback all apply immediately. The goal here is not flashy UI, but a steadier workspace for long file-tree sessions." => "El idioma, el tema y las fuentes de respaldo se aplican al instante. El objetivo no es una interfaz llamativa, sino un espacio de trabajo más estable para sesiones largas sobre el árbol de archivos.",
    "Interface Language" => "Idioma de la interfaz",
    "Manual selection overrides automatic locale detection." => "La selección manual reemplaza la detección automática del idioma del sistema.",
    "Interface Theme" => "Tema de la interfaz",
    "Dark is better for long analysis sessions; light stays restrained and low contrast." => "El modo oscuro funciona mejor en sesiones largas de análisis; el claro se mantiene sobrio y con bajo contraste.",
    "Light" => "Claro",
    "Dark" => "Oscuro",
    "Advanced Tools" => "Herramientas avanzadas",
    "Keeps history, errors, and diagnostics behind a secondary entry. Most cleanup flows do not need them by default." => "Mantiene el historial, los errores y el diagnóstico detrás de una entrada secundaria. La mayoría de los flujos de limpieza no los necesitan por defecto.",
    "Enabled" => "Activado",
    "Hidden" => "Oculto",
    "DirOtter is not a loud cleaner UI. It is a quieter, analytical workspace with restrained emphasis." => "DirOtter no es una interfaz ruidosa de limpieza. Es un espacio analítico más sobrio y calmado.",
    "Current Mode" => "Modo actual",
    "Localization Notes" => "Notas de localización",
    "The app now prefers CJK-capable system fallback fonts (Windows prioritizes Microsoft YaHei / DengXian) so Chinese labels do not render as tofu boxes." => "La aplicación ahora prioriza fuentes de respaldo del sistema compatibles con CJK (Windows prioriza Microsoft YaHei / DengXian) para que las etiquetas en chino no aparezcan como cuadros.",
    "The first launch can still infer language from the system locale, but the manual choice here overrides auto-detection." => "El primer inicio puede inferir el idioma desde la configuración regional, pero la elección manual aquí reemplaza esa detección.",
    "Why DirOtter" => "Por qué DirOtter",
    "Dir points to directories, while Otter brings a clever, tidy, exploratory character. The product should feel like a calm storage analyzer, not a noisy junk cleaner." => "Dir apunta a directorios, mientras que Otter aporta una idea de inteligencia, orden y exploración. El producto debe sentirse como un analizador de almacenamiento sereno, no como un limpiador ruidoso.",
    "Stop Scan" => "Detener escaneo",
    "Cancel" => "Cancelar",
    "Inspector" => "Inspector",
    "Details for the current selection" => "Detalles de la selección actual",
    "Name" => "Nombre",
    "Directory" => "Directorio",
    "File" => "Archivo",
    "Path" => "Ruta",
    "Full path available on hover" => "Ruta completa disponible al pasar el cursor",
    "Size" => "Tamaño",
    "No file or folder selected yet. Pick one from the live list, history, errors, or treemap." => "Aún no hay ningún archivo o carpeta seleccionado. Elige uno de la lista en vivo, el historial, los errores o la vista de resultados.",
    "No file or folder is selected yet. Pick one from the live list, result view, or another page." => "Todavía no hay ningún archivo o carpeta seleccionado. Elige uno de la lista en vivo, la vista de resultados o cualquier otra página.",
    "Background Task: Recycle Bin" => "Tarea en segundo plano: papelera",
    "Background Task: Permanent Delete" => "Tarea en segundo plano: eliminación permanente",
    "Deletion is running in the background. You can keep browsing results, but new delete actions stay locked for now." => "La eliminación se está ejecutando en segundo plano. Puedes seguir revisando resultados, pero las nuevas eliminaciones siguen bloqueadas por ahora.",
    "Target" => "Destino",
    "items in flight" => "elementos en curso",
    "Elapsed" => "Tiempo transcurrido",
    "Recycle-bin delete" => "Eliminar a la papelera",
    "Permanent delete" => "Eliminación permanente",
    "Quick Actions" => "Acciones rápidas",
    "Delete directly from the inspector instead of jumping to a separate page." => "Elimina directamente desde el inspector en lugar de abrir una página separada.",
    "Release Memory" => "Liberar memoria",
    "Release System Memory" => "Liberar memoria del sistema",
    "Clear transient app caches and try to shrink the current process. Disabled during scan or delete work." => "Limpia las cachés temporales de la aplicación e intenta reducir el proceso actual. Se desactiva durante el escaneo o la eliminación.",
    "Uses Windows-supported memory trimming to shrink large interactive processes and, when allowed, trim the system file cache." => "Usa las capacidades admitidas por Windows para reducir procesos interactivos con alto consumo y, cuando los permisos lo permiten, recortar la caché de archivos del sistema.",
    "Memory release stays disabled while scan or delete tasks are active so the current work is not interrupted." => "La liberación de memoria permanece desactivada mientras haya tareas de escaneo o eliminación activas para no interrumpir el trabajo actual.",
    "System memory release is running in the background. The UI stays responsive and will show the before/after result automatically." => "La liberación de memoria del sistema se está ejecutando en segundo plano. La interfaz seguirá respondiendo y mostrará automáticamente el resultado antes/después.",
    "Open File Location" => "Abrir ubicación",
    "Opened the target location in the system file manager." => "La ubicación del destino se abrió en el explorador de archivos del sistema.",
    "Failed to open location" => "No se pudo abrir la ubicación",
    "Move to Recycle Bin" => "Mover a la papelera",
    "Delete Permanently" => "Eliminar permanentemente",
    "A background delete task is running. You can keep browsing, but new delete actions stay disabled until it finishes." => "Hay una eliminación en segundo plano en curso. Puedes seguir navegando, pero las nuevas eliminaciones estarán desactivadas hasta que termine.",
    "Select a file or folder from a list, treemap, history, or errors first." => "Selecciona primero un archivo o carpeta desde una lista, la vista de resultados, el historial o los errores.",
    "Select a file or folder from a list, result view, or another page first." => "Selecciona primero un archivo o carpeta desde una lista, la vista de resultados o cualquier otra página.",
    "Opened Location" => "Ubicación abierta",
    "Open Location Failed" => "Falló al abrir la ubicación",
    "Last Action" => "Última acción",
    "Moved to recycle bin" => "Movido a la papelera",
    "Result" => "Resultado",
    "Failure" => "Fallo",
    "Workspace Context" => "Contexto del espacio de trabajo",
    "Root" => "Raíz",
    "Current scan target" => "Destino actual del escaneo",
    "Source" => "Origen",
    "None" => "Ninguno",
    "Selection source" => "Origen de la selección",
    "Confirm Permanent Delete" => "Confirmar eliminación permanente",
    "This action deletes the file or folder directly without using the recycle bin." => "Esta acción elimina directamente el archivo o la carpeta sin usar la papelera.",
    "Recommendation: prefer recycle-bin deletion for routine cleanup. Use permanent delete only when you are certain." => "Recomendación: prioriza la papelera para la limpieza rutinaria. Usa la eliminación permanente solo cuando estés seguro.",
    "Cleanup Details" => "Detalles de limpieza",
    "Safe items are selected by default and warning items stay unchecked. For red items, click the row and use Open Selected Location for manual review." => "Los elementos seguros se seleccionan por defecto y los de advertencia quedan sin marcar. Para los elementos rojos, haz clic en la fila y usa Abrir ubicación seleccionada para revisarlos manualmente.",
    "days unused" => "días sin uso",
    "Score" => "Puntuación",
    "Close" => "Cerrar",
    "Confirm Cleanup" => "Confirmar limpieza",
    "Suggested items will be moved to the recycle bin first instead of being deleted permanently." => "Los elementos sugeridos se moverán primero a la papelera en lugar de eliminarse permanentemente.",
    "Task" => "Tarea",
    "Rule-driven cleanup" => "Limpieza guiada por reglas",
    "Items" => "Elementos",
    "Will be moved to recycle bin" => "Se moverán a la papelera",
    "Estimated Reclaim" => "Recuperación estimada",
    "Actual reclaim depends on execution results" => "La recuperación real depende del resultado de la ejecución",
    "more items not shown" => "más elementos no mostrados",
    "Start Cleanup" => "Iniciar limpieza",
    "errors" => "errores",
    "used" => "usado",
    "items" => "elementos",
    "Moving to Recycle Bin in Background" => "Moviendo a la papelera en segundo plano",
    "Deleting Permanently in Background" => "Eliminando permanentemente en segundo plano",
    "You can keep browsing scan results. The UI will synchronize automatically when deletion finishes." => "Puedes seguir revisando los resultados del escaneo. La interfaz se sincronizará automáticamente cuando termine la eliminación.",
    "System is processing the delete request" => "El sistema está procesando la solicitud de eliminación",
    "Finalizing final results..." => "Finalizando resultados...",
    "Finalizing Final Results" => "Finalizando resultados",
    "Manual path (optional)" => "Ruta manual (opcional)",
    "Start with a drive button first. Only type a manual path when you need a subfolder." => "Empieza primero con un botón de unidad. Escribe una ruta manual solo cuando necesites una subcarpeta.",
    "Scanning finds storage hotspots. Use the separate memory action in Quick Actions for memory release." => "El escaneo encuentra los puntos críticos de almacenamiento. Usa la acción de memoria separada en Acciones rápidas para liberar memoria.",
    "There is no scan result to save yet." => "Todavía no hay resultados de escaneo para guardar.",
    "Saved the current snapshot manually." => "La instantánea actual se guardó manualmente.",
    "Failed to save snapshot" => "No se pudo guardar la instantánea",
    "Recorded the current scan summary manually." => "El resumen del escaneo actual se registró manualmente.",
    "Failed to record scan history" => "No se pudo registrar el historial del escaneo",
    "Exported the errors CSV." => "Se exportó el CSV de errores.",
    "Failed to export errors CSV" => "No se pudo exportar el CSV de errores",
    "Exported the diagnostics bundle." => "Se exportó el paquete de diagnóstico.",
    "One-Tap Boost" => "Aceleración con un clic",
    "Boost Now (Recommended)" => "Acelerar ahora (recomendado)",
    "Start Boost Scan" => "Iniciar escaneo para acelerar",
    "Review Boost Suggestions" => "Ver sugerencias para acelerar",
    "Review Largest Items" => "Ver elementos más grandes",
    "Advanced Maintenance" => "Mantenimiento avanzado",
    "The safest and most direct one-tap boost right now is cache cleanup, with about" => "La aceleración con un clic más segura y directa en este momento es limpiar caché, con aproximadamente",
    "Run a scan first so DirOtter can identify safe cache and the largest cleanup targets." => "Primero ejecuta un escaneo para que DirOtter pueda identificar cachés seguros y los mayores objetivos de limpieza.",
    "Potential system-slowing storage targets were found, but they still need your confirmation before execution." => "Se encontraron elementos de almacenamiento que podrían ralentizar el sistema, pero aún necesitan tu confirmación antes de ejecutarse.",
    "No safe one-tap boost stands out right now. Starting from the largest folders and files is usually the most effective next step." => "Ahora mismo no destaca ninguna aceleración segura con un clic. Empezar por las carpetas y archivos más grandes suele ser el siguiente paso más efectivo.",
    "There is no obvious one-tap boost action right now. Start from the largest folders and files below." => "Ahora mismo no hay una acción evidente de aceleración con un clic. Empieza por las carpetas y archivos más grandes de abajo.",
    "These actions are mainly for diagnostics and recovery. They are not part of the normal one-tap speed path for everyday users." => "Estas acciones son principalmente para diagnóstico y recuperación. No forman parte de la ruta normal de aceleración con un clic para usuarios cotidianos.",
    "Clean Interrupted Cleanup Area" => "Limpiar área de limpieza interrumpida",
    "Optimize DirOtter Memory" => "Optimizar memoria de DirOtter",
    "Clean Up Staging" => "Limpiar staging",
    "Maintenance Done" => "Mantenimiento completado",
    "Maintenance Failed" => "Error de mantenimiento",
    "Working set reclaimed about" => "Conjunto de trabajo recuperado aprox.",
    "System free memory increased by about" => "La memoria libre del sistema aumentó aproximadamente",
    "Trimmed processes" => "Procesos reducidos",
    "Scanned candidates" => "Candidatos analizados",
    "System file cache trimmed" => "Caché de archivos del sistema recortada",
    "System memory release failed" => "La liberación de memoria del sistema falló",
    "A disk snapshot was saved first, so the result can be reloaded later." => "Primero se guardó una instantánea en disco, para que el resultado pueda recargarse más tarde.",
    "Cleared the current result and optimized DirOtter memory usage." => "Se limpió el resultado actual y se optimizó el uso de memoria de DirOtter.",
    "Cleared current results, but Windows working-set trimming failed" => "Se limpiaron los resultados actuales, pero falló la reducción del conjunto de trabajo de Windows",
    "system free" => "memoria libre del sistema",
    "load" => "carga",
    "Cleared the current result and requested DirOtter memory trimming." => "Se limpió el resultado actual y se solicitó reducir la memoria de DirOtter.",
    "Cleared current results, but memory trimming failed" => "Se limpiaron los resultados actuales, pero falló la reducción de memoria",
    "Manually cleaned remaining staging items." => "Se limpiaron manualmente los elementos restantes de staging.",
    "Failed to clean staging" => "No se pudo limpiar staging",
    "Cleaned leftover items from the interrupted cleanup area." => "Se limpiaron los elementos restantes del área de limpieza interrumpida.",
    "Failed to clean the interrupted cleanup area" => "No se pudo limpiar el área de limpieza interrumpida",
    "Wait for scan or delete tasks to finish before releasing memory." => "Espera a que terminen las tareas de escaneo o eliminación antes de liberar memoria.",
    "Memory release completed: transient caches were cleared and the current process was trimmed." => "La liberación de memoria se completó: se limpiaron las cachés temporales y se redujo el proceso actual.",
    "There is no additional application-side memory to release right now." => "Ahora mismo no hay memoria adicional del lado de la aplicación para liberar.",
    "Complete a scan first before using this result view. DirOtter does not auto-load old cached results here, so the UI stays responsive." => "Completa primero un escaneo antes de usar esta vista de resultados. DirOtter no carga aquí automáticamente resultados antiguos en caché para mantener la interfaz receptiva.",
    "Failure Details" => "Detalles del fallo",
    "These items failed to execute. Full paths, failure reasons, and suggestions are listed here." => "Estos elementos no pudieron ejecutarse. Aquí se muestran las rutas completas, los motivos del fallo y las sugerencias.",
    "Close the details and return to the inspector summary." => "Cierra este panel y vuelve al resumen del inspector.",
    "Progress" => "Progreso",
    "Current Item" => "Elemento actual",
    "Current item" => "Elemento actual",
    "Items In This Cleanup" => "Elementos incluidos en esta limpieza",
    "Delete action failed. Review the failure reason and retry after checking the target state." => "La eliminación falló. Revisa el motivo del fallo y vuelve a intentarlo después de comprobar el estado del objetivo.",
    "Background Task: Fast Cleanup" => "Tarea en segundo plano: limpieza rápida",
    "Instant move, background purge" => "Movimiento instantáneo, purga en segundo plano",
    "Will be staged for background cleanup" => "Se moverá al área de limpieza en segundo plano",
    "Will move to the system recycle bin" => "Se moverá a la papelera del sistema",
    "Disk space will continue to be reclaimed in the background" => "El espacio en disco se seguirá recuperando en segundo plano",
    "Clean Now" => "Limpiar ahora",
    "Fast Cleanup" => "Limpieza rápida",
    "Fast cleanup" => "Limpieza rápida",
    "Select Safe" => "Seleccionar seguros",
    "Clear Selected" => "Limpiar selección",
    "Open Selected" => "Abrir selección",
    "Background Task: Sync Results" => "Tarea en segundo plano: sincronizar resultados",
    "Deletion has finished. The result view and cleanup suggestions are synchronizing in the background and will refresh automatically." => "La eliminación ha terminado. La vista de resultados y las sugerencias de limpieza se están sincronizando en segundo plano y se actualizarán automáticamente.",
    "items processed" => "elementos procesados",
    "Result Sync" => "Sincronización de resultados",
    "Syncing in background" => "Sincronizando en segundo plano",
    "Synchronizing the result view and cleanup suggestions after deletion" => "Sincronizando la vista de resultados y las sugerencias de limpieza después de la eliminación",
    "Synchronizing Cleanup Results" => "Sincronizando resultados de limpieza",
    "Deletion finished. The result view and cleanup suggestions are being synchronized in the background." => "La eliminación terminó. La vista de resultados y las sugerencias de limpieza se están sincronizando en segundo plano.",
    "System is synchronizing post-delete results" => "El sistema está sincronizando los resultados posteriores a la eliminación",
    "Result View Is Waiting For Cleanup Sync" => "La vista de resultados está esperando a que termine la sincronización de limpieza",
    "Background deletion or result synchronization is still running. DirOtter will resume the result view after it finishes so snapshot loading and result rebuilding do not block the UI thread." => "La eliminación en segundo plano o la sincronización de resultados sigue en curso. DirOtter reanudará la vista de resultados cuando termine para que la carga de instantáneas y la reconstrucción de resultados no bloqueen el hilo de la interfaz.",
    "Loading Saved Result Snapshot" => "Cargando instantánea guardada de resultados",
    "DirOtter is loading the saved result snapshot in the background. The lightweight result view will open automatically when it is ready, without decompressing or rebuilding the whole result tree on the current UI frame." => "DirOtter está cargando en segundo plano la instantánea guardada de resultados. La vista ligera de resultados se abrirá automáticamente cuando esté lista, sin descomprimir ni reconstruir todo el árbol de resultados en el hilo actual de la interfaz.",
    "Permission Denied" => "Permiso denegado",
    "Open the full failed-item list with paths, reasons, and suggestions." => "Abre la lista completa de elementos fallidos con rutas, motivos y sugerencias.",
    "Execution" => "Ejecución",
    "Still Failed After Retries" => "Sigue fallando tras los reintentos",
    "Blocked by Safety Policy" => "Bloqueado por la política de seguridad",
    "I/O Failure" => "Error de E/S",
    "Target Missing" => "Objetivo no encontrado",
    "Platform Unavailable" => "Plataforma no disponible",
    "Operation Not Supported" => "Operación no compatible",
    "State Changed Before Execution" => "El estado cambió antes de ejecutar",
    "Unsupported Target Type" => "Tipo de objetivo no compatible",
    "The system rejected this delete request, usually because of missing privileges or target protection." => "El sistema rechazó esta solicitud de eliminación, normalmente por falta de privilegios o porque el objetivo está protegido.",
    "This path matched the current safety rules, so deletion was not executed directly." => "Esta ruta coincidió con las reglas de seguridad actuales, así que la eliminación directa no se ejecutó.",
    "The system already retried this operation" => "El sistema ya reintentó esta operación",
    "times, but it still did not succeed." => "veces, pero aun así no tuvo éxito.",
    "The execution hit an I/O issue, commonly due to file locks, transient handles, or permission transitions." => "La ejecución encontró un problema de E/S, normalmente causado por archivos bloqueados, identificadores transitorios o cambios de permisos.",
    "The target disappeared from disk before execution completed." => "El objetivo desapareció del disco antes de que terminara la ejecución.",
    "The current platform or delete mode cannot complete this request." => "La plataforma actual o el modo de eliminación no pueden completar esta solicitud.",
    "The disk state changed between precheck and actual execution." => "El estado del disco cambió entre la comprobación previa y la ejecución real.",
    "This object is not a regular file or directory supported by the current delete flow." => "Este objeto no es un archivo o carpeta normal compatible con el flujo de eliminación actual.",
    "This delete did not complete successfully. Review the suggestion below and re-check the target state." => "Esta eliminación no se completó correctamente. Revisa la sugerencia de abajo y vuelve a comprobar el estado del objetivo.",
    "failed, view details" => "fallos, ver detalles",
    "Suggested Next Step" => "Siguiente paso sugerido",
    "Technical Detail" => "Detalle técnico",
});

pub(crate) fn translate_fr(en: &str) -> &str {
    if let Some(local) = lookup_fr(en) {
        local
    } else {
        generated_group0::translate_fr(en)
    }
}

pub(crate) fn translate_es(en: &str) -> &str {
    if let Some(local) = lookup_es(en) {
        local
    } else {
        generated_group0::translate_es(en)
    }
}

#[cfg(test)]
pub(crate) fn has_translation_fr(en: &str) -> bool {
    lookup_fr(en).is_some() || generated_group0::has_translation_fr(en)
}

#[cfg(test)]
pub(crate) fn has_translation_es(en: &str) -> bool {
    lookup_es(en).is_some() || generated_group0::has_translation_es(en)
}
