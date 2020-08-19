//---------------------------------------------------------------------------//
// Copyright (c) 2017-2020 Ismael Gutiérrez González. All rights reserved.
//
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
//
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

/*!
Module with the slots for Table Views.
!*/

use qt_widgets::SlotOfQPoint;
use qt_widgets::QFileDialog;
use qt_widgets::q_file_dialog::AcceptMode;
use qt_widgets::SlotOfIntSortOrder;
use qt_widgets::q_header_view::ResizeMode;

use qt_gui::QBrush;
use qt_gui::QCursor;
use qt_gui::SlotOfQStandardItem;

use qt_core::QModelIndex;
use qt_core::QItemSelection;
use qt_core::QSignalBlocker;
use qt_core::{SlotOfBool, SlotOfInt, Slot, SlotOfQString, SlotOfQItemSelectionQItemSelection, SlotOfQModelIndex};

use cpp_core::Ref;

use std::sync::atomic::Ordering;
use std::path::PathBuf;

use rpfm_lib::packedfile::table::Table;

use crate::app_ui::AppUI;
use crate::ffi::*;
use crate::global_search_ui::GlobalSearchUI;
use crate::packfile_contents_ui::PackFileContentsUI;
use crate::packedfile_views::utils::set_modified;
use crate::pack_tree::*;
use crate::utils::atomic_from_mut_ptr;
use crate::utils::show_dialog;
use crate::UI_STATE;

use super::utils::*;
use super::*;

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

/// This struct contains the slots of the view of an Table PackedFile.
pub struct TableViewSlots {
    pub filter_line_edit: SlotOfQString<'static>,
    pub filter_column_selector: SlotOfInt<'static>,
    pub filter_case_sensitive_button: Slot<'static>,
    pub toggle_lookups: SlotOfBool<'static>,
    pub sort_order_column_changed: SlotOfIntSortOrder<'static>,
    pub show_context_menu: SlotOfQPoint<'static>,
    pub context_menu_enabler: SlotOfQItemSelectionQItemSelection<'static>,
    pub item_changed: SlotOfQStandardItem<'static>,
    pub add_rows: Slot<'static>,
    pub insert_rows: Slot<'static>,
    pub delete_rows: Slot<'static>,
    pub clone_and_append: Slot<'static>,
    pub clone_and_insert: Slot<'static>,
    pub copy: Slot<'static>,
    pub copy_as_lua_table: Slot<'static>,
    pub paste: Slot<'static>,
    pub invert_selection: Slot<'static>,
    pub reset_selection: Slot<'static>,
    pub rewrite_selection: Slot<'static>,
    pub save: Slot<'static>,
    pub undo: Slot<'static>,
    pub redo: Slot<'static>,
    pub import_tsv: SlotOfBool<'static>,
    pub export_tsv: SlotOfBool<'static>,
    pub smart_delete: Slot<'static>,
    pub resize_columns: Slot<'static>,
    pub sidebar: SlotOfBool<'static>,
    pub search: SlotOfBool<'static>,
    pub hide_show_columns: Vec<SlotOfInt<'static>>,
    pub freeze_columns: Vec<SlotOfInt<'static>>,
    pub search_search: Slot<'static>,
    pub search_prev_match: Slot<'static>,
    pub search_next_match: Slot<'static>,
    pub search_replace_current: Slot<'static>,
    pub search_replace_all: Slot<'static>,
    pub search_close: Slot<'static>,
    pub open_subtable: SlotOfQModelIndex<'static>,
}

//-------------------------------------------------------------------------------//
//                             Implementations
//-------------------------------------------------------------------------------//

/// Implementation for `TableViewSlots`.
impl TableViewSlots {

    /// This function creates the entire slot pack for images.
    pub unsafe fn new(
        view: &TableViewRaw,
        global_search_ui: GlobalSearchUI,
        mut pack_file_contents_ui: PackFileContentsUI,
        mut app_ui: AppUI,
        packed_file_path: Option<Arc<RwLock<Vec<String>>>>,

    ) -> Self {

        // When we want to filter the table...
        let filter_line_edit = SlotOfQString::new(clone!(
            mut view => move |_| {
            view.filter_table();
        }));

        let filter_column_selector = SlotOfInt::new(clone!(
            mut view => move |_| {
            view.filter_table();
        }));

        let filter_case_sensitive_button = Slot::new(clone!(
            mut view => move || {
            view.filter_table();
        }));

        // When we want to toggle the lookups on and off.
        let toggle_lookups = SlotOfBool::new(clone!(
            view => move |_| {
            view.toggle_lookups();
        }));

        let sort_order_column_changed = SlotOfIntSortOrder::new(clone!(
            view => move |column, _| {
                sort_column(view.table_view_primary, column, view.column_sort_state.clone());
            }
        ));

        // When we want to show the context menu.
        let show_context_menu = SlotOfQPoint::new(clone!(
            mut view => move |_| {
            view.context_menu.exec_1a_mut(&QCursor::pos_0a());
        }));

        // When we want to trigger the context menu update function.
        let context_menu_enabler = SlotOfQItemSelectionQItemSelection::new(clone!(
            mut view => move |_,_| {
            view.context_menu_update();
        }));

        // When we want to respond to a change in one item in the model.
        let item_changed = SlotOfQStandardItem::new(clone!(
            mut pack_file_contents_ui,
            mut view => move |item| {

                // If we are NOT UNDOING, paint the item as edited and add the edition to the undo list.
                if !view.undo_lock.load(Ordering::SeqCst) {
                    let item_old = view.undo_model.item_2a(item.row(), item.column());

                    // Only trigger this if the values are actually different. Checkable cells are tricky. Nested cells an go to hell.
                    if (item_old.text().compare_q_string(item.text().as_ref()) != 0 || item_old.check_state() != item.check_state()) ||
                        item_old.data_1a(ITEM_IS_SEQUENCE).to_bool() && 0 != item_old.data_1a(ITEM_SEQUENCE_DATA).to_string().compare_q_string(&item.data_1a(ITEM_SEQUENCE_DATA).to_string()) {
                        let mut edition = Vec::with_capacity(1);
                        edition.push(((item.row(), item.column()), atomic_from_mut_ptr((&*item_old).clone())));
                        let operation = TableOperations::Editing(edition);
                        view.history_undo.write().unwrap().push(operation);
                        view.history_redo.write().unwrap().clear();

                        {
                            // We block the saving for painting, so this doesn't get rettriggered again.
                            let mut blocker = QSignalBlocker::from_q_object(view.table_model);
                            let color = get_color_modified();
                            let mut item = item;
                            item.set_background(&QBrush::from_q_color(color.as_ref().unwrap()));
                            blocker.unblock();
                        }

                        // For pasting, or really any heavy operation, only do these tasks the last iteration of the operation.
                        if !view.save_lock.load(Ordering::SeqCst) {
                            update_undo_model(view.table_model, view.undo_model);
                            view.context_menu_update();
                            if let Some(ref packed_file_path) = packed_file_path {
                                TableSearch::update_search(&mut view);
                                set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                            }
                        }
                    }
                }

                // If we have the dependency stuff enabled, check if it's a valid reference.
                let column = item.column();
                match *view.packed_file_type {
                    PackedFileType::DB => {
                        if SETTINGS.read().unwrap().settings_bool["use_dependency_checker"] {
                            if view.get_ref_table_definition().get_fields_processed()[column as usize].get_is_reference().is_some() {
                                check_references(column, item, &view.dependency_data.read().unwrap(), *view.packed_file_type);
                            }
                        }
                    }
                    PackedFileType::DependencyPackFilesList => {
                        check_references(column, item, &view.dependency_data.read().unwrap(), *view.packed_file_type);
                    }
                    _ => {}
                }
            }
        ));

        // When you want to append a row to the table...
        let add_rows = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
                view.append_rows(false);
                if let Some(ref packed_file_path) = view.packed_file_path {
                    set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                }
            }
        ));

        // When you want to insert a row in a specific position of the table...
        let insert_rows = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
                view.insert_rows(false);
                if let Some(ref packed_file_path) = view.packed_file_path {
                    set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                }
            }
        ));

        // When you want to delete one or more rows...
        let delete_rows = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {

                // Get all the selected rows.
                let selection = view.table_view_primary.selection_model().selection();
                let indexes = view.table_filter.map_selection_to_source(&selection).indexes();
                let indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
                let mut rows_to_delete: Vec<i32> = indexes_sorted.iter().filter_map(|x| if x.is_valid() { Some(x.row()) } else { None }).collect();

                // Dedup the list and reverse it.
                rows_to_delete.sort();
                rows_to_delete.dedup();
                rows_to_delete.reverse();
                let rows_splitted = delete_rows(view.table_model, &rows_to_delete);

                // If we deleted something, try to save the PackedFile to the main PackFile.
                if !rows_to_delete.is_empty() {
                    view.history_undo.write().unwrap().push(TableOperations::RemoveRows(rows_splitted));
                    view.history_redo.write().unwrap().clear();
                    update_undo_model(view.table_model, view.undo_model);
                    if let Some(ref packed_file_path) = view.packed_file_path {
                        set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                    }
                }
            }
        ));

        // When you want to clone and insert one or more rows.
        let clone_and_append = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
            view.append_rows(true);
            if let Some(ref packed_file_path) = view.packed_file_path {
                set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
            }
        }));

        // When you want to clone and append one or more rows.
        let clone_and_insert = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
            view.insert_rows(true);
            if let Some(ref packed_file_path) = view.packed_file_path {
                set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
            }
        }));

        // When you want to copy one or more cells.
        let copy = Slot::new(clone!(
            view => move || {
            view.copy_selection();
        }));

        // When you want to copy a table as a lua table.
        let copy_as_lua_table = Slot::new(clone!(
            view => move || {
            view.copy_selection_as_lua_table();
        }));

        // When you want to copy one or more cells.
        let paste = Slot::new(clone!(
            mut view => move || {
            view.paste();
        }));

        // When we want to invert the selection of the table.
        let invert_selection = Slot::new(clone!(
            mut view => move || {
            let rows = view.table_filter.row_count_0a();
            let columns = view.table_filter.column_count_0a();
            if rows > 0 && columns > 0 {
                let mut selection_model = view.table_view_primary.selection_model();
                let first_item = view.table_filter.index_2a(0, 0);
                let last_item = view.table_filter.index_2a(rows - 1, columns - 1);
                let selection = QItemSelection::new_2a(&first_item, &last_item);
                selection_model.select_q_item_selection_q_flags_selection_flag(&selection, QFlags::from(SelectionFlag::Toggle));
            }
        }));

        // When we want to reset the selected items of the table to their original value.
        let reset_selection = Slot::new(clone!(
            mut view => move || {
            view.reset_selection();
        }));

        // When we want to rewrite the selected items using a formula.
        let rewrite_selection = Slot::new(clone!(
            mut view => move || {
            view.rewrite_selection();
        }));

        // When we want to save the contents of the UI to the backend...
        //
        // NOTE: in-edition saves to backend are only triggered when the GlobalSearch has search data, to keep it updated.
        let save = Slot::new(clone!(
            view => move || {
            if !UI_STATE.get_global_search_no_lock().pattern.is_empty() {
                if let Some(ref packed_file_path) = view.packed_file_path {
                    if let Some(packed_file) = UI_STATE.get_open_packedfiles().iter().find(|x| *x.get_ref_path() == *packed_file_path.read().unwrap()) {
                        if let Err(error) = packed_file.save(&mut app_ui, global_search_ui, &mut pack_file_contents_ui) {
                            show_dialog(view.table_view_primary, error, false);
                        }
                    }
                }
            }
        }));

        // When we want to undo the last action.
        let undo = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
                view.undo_redo(true, 0);
                update_undo_model(view.table_model, view.undo_model);
                view.context_menu_update();
                if view.history_undo.read().unwrap().is_empty() {
                    if let Some(ref packed_file_path) = view.packed_file_path {
                        set_modified(false, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                    }
                }
            }
        ));

        // When we want to redo the last undone action.
        let redo = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
                view.undo_redo(false, 0);
                update_undo_model(view.table_model, view.undo_model);
                view.context_menu_update();
                if let Some(ref packed_file_path) = view.packed_file_path {
                    set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                }
            }
        ));

        // When we want to import a TSV file.
        let import_tsv = SlotOfBool::new(clone!(
            mut pack_file_contents_ui,
            mut view => move |_| {

                // For now only import if this is the parent table.
                if let Some(ref packed_file_path) = view.packed_file_path {

                    // Create a File Chooser to get the destination path and configure it.
                    let mut file_dialog = QFileDialog::from_q_widget_q_string(
                        view.table_view_primary,
                        &qtr("tsv_select_title"),
                    );

                    file_dialog.set_name_filter(&QString::from_std_str("TSV Files (*.tsv)"));

                    // Run it and, if we receive 1 (Accept), try to import the TSV file.
                    if file_dialog.exec() == 1 {
                        let path = PathBuf::from(file_dialog.selected_files().at(0).to_std_string());

                        CENTRAL_COMMAND.send_message_qt(Command::ImportTSV((packed_file_path.read().unwrap().to_vec(), path)));
                        let response = CENTRAL_COMMAND.recv_message_qt_try();
                        match response {
                            Response::TableType(data) => {
                                let old_data = view.get_copy_of_table();

                                view.undo_lock.store(true, Ordering::SeqCst);
                                load_data(
                                    view.table_view_primary,
                                    view.table_view_frozen,
                                    &view.get_ref_table_definition(),
                                    &view.dependency_data,
                                    &data
                                );

                                let table_name = match data {
                                    TableType::DB(_) => packed_file_path.read().unwrap().get(1).cloned(),
                                    _ => None,
                                };

                                build_columns(
                                    view.table_view_primary,
                                    Some(view.table_view_frozen),
                                    &view.get_ref_table_definition(),
                                    table_name.as_ref()
                                );

                                view.undo_lock.store(false, Ordering::SeqCst);

                                view.history_undo.write().unwrap().push(TableOperations::ImportTSV(old_data));
                                view.history_redo.write().unwrap().clear();
                                update_undo_model(view.table_model, view.undo_model);
                                set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                            },
                            Response::Error(error) => return show_dialog(view.table_view_primary, error, false),
                            _ => panic!("{}{:?}", THREADS_COMMUNICATION_ERROR, response),
                        }

                        //unsafe { update_search_stuff.as_mut().unwrap().trigger(); }
                        view.context_menu_update();
                    }
                }
            }
        ));

        // When we want to export the table as a TSV File.
        let export_tsv = SlotOfBool::new(clone!(
            view => move |_| {

                if let Some(ref packed_file_path) = view.packed_file_path {

                    // Create a File Chooser to get the destination path and configure it.
                    let mut file_dialog = QFileDialog::from_q_widget_q_string(
                        view.table_view_primary,
                        &qtr("tsv_export_title")
                    );

                    file_dialog.set_accept_mode(AcceptMode::AcceptSave);
                    file_dialog.set_confirm_overwrite(true);
                    file_dialog.set_name_filter(&QString::from_std_str("TSV Files (*.tsv)"));
                    file_dialog.set_default_suffix(&QString::from_std_str("tsv"));

                    // Run it and, if we receive 1 (Accept), export the DB Table, saving it's contents first.
                    if file_dialog.exec() == 1 {

                        let path = PathBuf::from(file_dialog.selected_files().at(0).to_std_string());
                        if let Some(packed_file) = UI_STATE.get_open_packedfiles().iter().find(|x| *x.get_ref_path() == *packed_file_path.read().unwrap()) {
                            if let Err(error) = packed_file.save(&mut app_ui, global_search_ui, &mut pack_file_contents_ui) {
                                return show_dialog(view.table_view_primary, error, false);
                            }
                        }

                        CENTRAL_COMMAND.send_message_qt(Command::ExportTSV((packed_file_path.read().unwrap().to_vec(), path)));
                        let response = CENTRAL_COMMAND.recv_message_qt_try();
                        match response {
                            Response::Success => return,
                            Response::Error(error) => return show_dialog(view.table_view_primary, error, false),
                            _ => panic!("{}{:?}", THREADS_COMMUNICATION_ERROR, response),
                        }
                    }
                }
            }
        ));

        // When we want to resize the columns depending on their contents...
        let resize_columns = Slot::new(clone!(view => move || {
            view.table_view_primary.horizontal_header().resize_sections(ResizeMode::ResizeToContents);
            if SETTINGS.read().unwrap().settings_bool["extend_last_column_on_tables"] {
                view.table_view_primary.horizontal_header().set_stretch_last_section(false);
                view.table_view_primary.horizontal_header().set_stretch_last_section(true);
            }
        }));

        // When you want to use the "Smart Delete" feature...
        let smart_delete = Slot::new(clone!(
            mut pack_file_contents_ui,
            mut view => move || {
                view.smart_delete();
                if let Some(ref packed_file_path) = view.packed_file_path {
                    set_modified(true, &packed_file_path.read().unwrap(), &mut app_ui, &mut pack_file_contents_ui);
                }
            }
        ));

        let sidebar = SlotOfBool::new(clone!(
            mut view => move |_| {
            match view.sidebar_scroll_area.is_visible() {
                true => view.sidebar_scroll_area.hide(),
                false => view.sidebar_scroll_area.show()
            }
        }));

        let search = SlotOfBool::new(clone!(
            mut view => move |_| {
            match view.search_widget.is_visible() {
                true => view.search_widget.hide(),
                false => view.search_widget.show()
            }
        }));

        let mut hide_show_columns = vec![];
        let mut freeze_columns = vec![];
        let mut fields = view.get_ref_table_definition().get_fields_processed().iter()
            .enumerate()
            .map(|(x, y)| (x as i32, y.get_ca_order()))
            .collect::<Vec<(i32, i16)>>();
        fields.sort_by(|(_, a), (_, b)| a.cmp(&b));
        let ca_order = fields.iter().map(|x| x.0).collect::<Vec<i32>>();

        for index in ca_order {
            let hide_show_slot = SlotOfInt::new(clone!(
                mut view => move |state| {
                    let state = state == 2;
                    view.table_view_primary.set_column_hidden(index, state);
                }
            ));

            let freeze_slot = SlotOfInt::new(clone!(
                mut view => move |_| {
                    toggle_freezer_safe(&mut view.table_view_primary, index);
                }
            ));

            hide_show_columns.push(hide_show_slot);
            freeze_columns.push(freeze_slot);
        }

        //------------------------------------------------------//
        // Slots related with the search panel.
        //------------------------------------------------------//

        let search_search = Slot::new(clone!(
            mut view => move || {
                TableSearch::search(&mut view);
            }
        ));

        let search_prev_match = Slot::new(clone!(
            mut view => move || {
                TableSearch::prev_match(&mut view);
            }
        ));

        let search_next_match = Slot::new(clone!(
            mut view => move || {
                TableSearch::next_match(&mut view);
            }
        ));

        let search_replace_current = Slot::new(clone!(
            mut view => move || {
                TableSearch::replace_current(&mut view);
            }
        ));

        let search_replace_all = Slot::new(clone!(
            mut view => move || {
                TableSearch::replace_all(&mut view);
            }
        ));

        let search_close = Slot::new(clone!(
            mut view => move || {
                view.search_widget.hide();
                view.table_view_primary.set_focus_0a();
            }
        ));

        let open_subtable = SlotOfQModelIndex::new(clone!(
            mut view => move |model_index| {
                if model_index.data_1a(ITEM_IS_SEQUENCE).to_bool() {
                    let data = model_index.data_1a(ITEM_SEQUENCE_DATA).to_string().to_std_string();
                    let table: Table = serde_json::from_str(&data).unwrap();
                    let table_data = match *view.packed_file_type {
                        PackedFileType::DB => TableType::DB(From::from(table)),
                        PackedFileType::Loc => TableType::Loc(From::from(table)),
                        PackedFileType::MatchedCombat => TableType::MatchedCombat(From::from(table)),
                        PackedFileType::AnimTable => TableType::AnimTable(From::from(table)),
                        PackedFileType::DependencyPackFilesList => unimplemented!("This should never happen, unless you messed up the schemas"),
                        _ => unimplemented!("You forgot to implement subtables for this kind of packedfile"),
                    };
                    if let Some(new_data) = open_subtable(
                        view.table_view_primary.static_upcast_mut(),
                        &app_ui,
                        &global_search_ui,
                        &pack_file_contents_ui,
                        table_data
                    ) {
                        view.table_filter.set_data_3a(
                            model_index,
                            &QVariant::from_q_string(&QString::from_std_str(new_data)),
                            ITEM_SEQUENCE_DATA
                        );
                    }
                }
            }
        ));

        // Return the slots, so we can keep them alive for the duration of the view.
        Self {
            filter_line_edit,
            filter_column_selector,
            filter_case_sensitive_button,
            toggle_lookups,
            sort_order_column_changed,
            show_context_menu,
            context_menu_enabler,
            item_changed,
            add_rows,
            insert_rows,
            delete_rows,
            clone_and_append,
            clone_and_insert,
            copy,
            copy_as_lua_table,
            paste,
            invert_selection,
            reset_selection,
            rewrite_selection,
            save,
            undo,
            redo,
            import_tsv,
            export_tsv,
            smart_delete,
            resize_columns,
            sidebar,
            search,
            hide_show_columns,
            freeze_columns,
            search_search,
            search_prev_match,
            search_next_match,
            search_replace_current,
            search_replace_all,
            search_close,
            open_subtable,
        }
    }
}
