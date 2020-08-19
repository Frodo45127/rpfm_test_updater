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
Module with all the code to deal with the raw version of the tables.
!*/

use qt_widgets::q_abstract_item_view::ScrollHint;
use qt_widgets::QAction;
use qt_widgets::QComboBox;
use qt_widgets::QDialog;
use qt_widgets::QGroupBox;
use qt_widgets::QLabel;
use qt_widgets::QLineEdit;
use qt_widgets::QPushButton;
use qt_widgets::QTableView;
use qt_widgets::QMenu;

use qt_gui::QBrush;
use qt_gui::QGuiApplication;
use qt_gui::QStandardItemModel;

use qt_core::CaseSensitivity;
use qt_core::QFlags;
use qt_core::QItemSelection;
use qt_core::QModelIndex;
use qt_core::QRegExp;
use qt_core::QSortFilterProxyModel;
use qt_core::QVariant;
use qt_core::QString;
use qt_core::Orientation;
use qt_core::q_item_selection_model::SelectionFlag;
use qt_core::QSignalBlocker;

use cpp_core::MutPtr;
use cpp_core::Ref;

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::sync::atomic::{AtomicBool, Ordering};

use rpfm_lib::schema::Definition;

use crate::utils::{atomic_from_mut_ptr, create_grid_layout, mut_ptr_from_atomic, log_to_status_bar};
use crate::pack_tree::*;
use super::*;

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

/// This struct contains the raw version of each pointer in `PackedFileTableView`, to be used when building the slots.
///
/// This is kinda a hack, because AtomicPtr cannot be copied, and we need a copy of the entire set of pointers available
/// for the construction of the slots. So we build this one, copy it for the slots, then move it into the `PackedFileTableView`.
#[derive(Clone)]
pub struct TableViewRaw {
    pub table_view_primary: MutPtr<QTableView>,
    pub table_view_frozen: MutPtr<QTableView>,
    pub table_filter: MutPtr<QSortFilterProxyModel>,
    pub table_model: MutPtr<QStandardItemModel>,
    //pub table_enable_lookups_button: MutPtr<QPushButton>,
    pub filter_case_sensitive_button: MutPtr<QPushButton>,
    pub filter_column_selector: MutPtr<QComboBox>,
    pub filter_line_edit: MutPtr<QLineEdit>,
    pub column_sort_state: Arc<RwLock<(i32, i8)>>,

    pub context_menu: MutPtr<QMenu>,
    pub context_menu_enabler: MutPtr<QAction>,
    pub context_menu_add_rows: MutPtr<QAction>,
    pub context_menu_insert_rows: MutPtr<QAction>,
    pub context_menu_delete_rows: MutPtr<QAction>,
    pub context_menu_clone_and_append: MutPtr<QAction>,
    pub context_menu_clone_and_insert: MutPtr<QAction>,
    pub context_menu_copy: MutPtr<QAction>,
    pub context_menu_copy_as_lua_table: MutPtr<QAction>,
    pub context_menu_paste: MutPtr<QAction>,
    pub context_menu_invert_selection: MutPtr<QAction>,
    pub context_menu_reset_selection: MutPtr<QAction>,
    pub context_menu_rewrite_selection: MutPtr<QAction>,
    pub context_menu_undo: MutPtr<QAction>,
    pub context_menu_redo: MutPtr<QAction>,
    pub context_menu_import_tsv: MutPtr<QAction>,
    pub context_menu_export_tsv: MutPtr<QAction>,
    pub context_menu_resize_columns: MutPtr<QAction>,
    pub context_menu_sidebar: MutPtr<QAction>,
    pub context_menu_search: MutPtr<QAction>,
    pub smart_delete: MutPtr<QAction>,

    pub sidebar_scroll_area: MutPtr<QScrollArea>,
    pub search_widget: MutPtr<QWidget>,

    pub search_search_line_edit: MutPtr<QLineEdit>,
    pub search_replace_line_edit: MutPtr<QLineEdit>,
    pub search_search_button: MutPtr<QPushButton>,
    pub search_replace_current_button: MutPtr<QPushButton>,
    pub search_replace_all_button: MutPtr<QPushButton>,
    pub search_close_button: MutPtr<QPushButton>,
    pub search_prev_match_button: MutPtr<QPushButton>,
    pub search_next_match_button: MutPtr<QPushButton>,
    pub search_matches_label: MutPtr<QLabel>,
    pub search_column_selector: MutPtr<QComboBox>,
    pub search_case_sensitive_button: MutPtr<QPushButton>,
    pub search_data: Arc<RwLock<TableSearch>>,

    pub dependency_data: Arc<RwLock<BTreeMap<i32, BTreeMap<String, String>>>>,
    pub table_definition: Arc<RwLock<Definition>>,
    pub packed_file_path: Option<Arc<RwLock<Vec<String>>>>,
    pub packed_file_type: Arc<PackedFileType>,

    pub save_lock: Arc<AtomicBool>,
    pub undo_lock: Arc<AtomicBool>,

    pub undo_model: MutPtr<QStandardItemModel>,
    pub history_undo: Arc<RwLock<Vec<TableOperations>>>,
    pub history_redo: Arc<RwLock<Vec<TableOperations>>>,
}

//-------------------------------------------------------------------------------//
//                             Implementations
//-------------------------------------------------------------------------------//

/// Implementation of `TableViewRaw`.
impl TableViewRaw {

    /// This function returns a reference to the definition of this table.
    pub fn get_ref_table_definition(&self) -> RwLockReadGuard<Definition> {
        self.table_definition.read().unwrap()
    }

    /// This function updates the state of the actions in the context menu.
    pub unsafe fn context_menu_update(&mut self) {

        // Turns out that this slot doesn't give the the amount of selected items, so we have to get them ourselfs.
        let indexes = self.table_filter.map_selection_to_source(&self.table_view_primary.selection_model().selection()).indexes();

        // If we have something selected, enable these actions.
        if indexes.count_0a() > 0 {
            self.context_menu_clone_and_append.set_enabled(true);
            self.context_menu_clone_and_insert.set_enabled(true);
            self.context_menu_copy.set_enabled(true);
            self.context_menu_copy_as_lua_table.set_enabled(true);
            self.context_menu_delete_rows.set_enabled(true);
            self.context_menu_rewrite_selection.set_enabled(true);
        }

        // Otherwise, disable them.
        else {
            self.context_menu_rewrite_selection.set_enabled(false);
            self.context_menu_clone_and_append.set_enabled(false);
            self.context_menu_clone_and_insert.set_enabled(false);
            self.context_menu_copy.set_enabled(false);
            self.context_menu_copy_as_lua_table.set_enabled(false);
            self.context_menu_delete_rows.set_enabled(false);
        }

        if !self.undo_lock.load(Ordering::SeqCst) {
            self.context_menu_undo.set_enabled(!self.history_undo.read().unwrap().is_empty());
            self.context_menu_redo.set_enabled(!self.history_redo.read().unwrap().is_empty());
        }
    }

    /// Function to filter the table.
    pub unsafe fn filter_table(&mut self) {

        let mut pattern = QRegExp::new_1a(&self.filter_line_edit.text());

        let column_name = self.filter_column_selector.current_text();
        for column in 0..self.table_model.column_count_0a() {
            if self.table_model.header_data_2a(column, Orientation::Horizontal).to_string().compare_q_string_case_sensitivity(&column_name, CaseSensitivity::CaseSensitive) == 0 {
                self.table_filter.set_filter_key_column(column);
                break;
            }
        }

        // Check if the filter should be "Case Sensitive".
        let case_sensitive = self.filter_case_sensitive_button.is_checked();

        if case_sensitive { pattern.set_case_sensitivity(CaseSensitivity::CaseSensitive); }
        else { pattern.set_case_sensitivity(CaseSensitivity::CaseInsensitive); }

        // Filter whatever it's in that column by the text we got.
        self.table_filter.set_filter_reg_exp_q_reg_exp(&pattern);
    }

    /// This function enables/disables showing the lookup values instead of the real ones in the columns that support it.
    pub unsafe fn toggle_lookups(&self) {
        /*
        if SETTINGS.lock().unwrap().settings_bool["disable_combos_on_tables"] {
            let enable_lookups = unsafe { self.table_enable_lookups_button.is_checked() };
            for (column, field) in table_definition.fields.iter().enumerate() {
                if let Some(data) = dependency_data.get(&(column as i32)) {
                    let mut list = QStringList::new(());
                    data.iter().map(|x| if enable_lookups { &x.1 } else { &x.0 }).for_each(|x| list.append(&QString::from_std_str(x)));
                    let list: *mut QStringList = &mut list;
                    unsafe { new_combobox_item_delegate_safe(self.table_view_primary as *mut QObject, column as i32, list as *const QStringList, true, field.max_length)};
                    unsafe { new_combobox_item_delegate_safe(self.table_view_frozen as *mut QObject, column as i32, list as *const QStringList, true, field.max_length)};
                }
            }
        }*/
    }

    /// This function resets the currently selected cells to their original value.
    pub unsafe fn reset_selection(&self) {

        // Get the current selection. As we need his visual order, we get it directly from the table/filter, NOT FROM THE MODEL.
        let indexes = self.table_view_primary.selection_model().selection().indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_visually(&mut indexes_sorted, self.table_view_primary);
        let indexes_sorted = get_real_indexes(&indexes_sorted, self.table_filter);

        let mut items_reverted = 0;
        for index in &indexes_sorted {
            if index.is_valid() {
                let mut item = self.table_model.item_from_index(index);
                if item.data_1a(ITEM_HAS_SOURCE_VALUE).to_bool() {
                    let original_data = item.data_1a(ITEM_SOURCE_VALUE);
                    let current_data = item.data_1a(2);
                    if original_data != current_data.as_ref() {
                        item.set_data_2a(&original_data, 2);
                        items_reverted += 1;
                    }
                }
            }
        }

        // Fix the undo history to have all the previous changed merged into one.
        if items_reverted > 0 {
            {
                let mut history_undo = self.history_undo.write().unwrap();
                let mut history_redo = self.history_redo.write().unwrap();

                let len = history_undo.len();
                let mut edits_data = vec![];
                {
                    let mut edits = history_undo.drain((len - items_reverted)..);
                    for edit in &mut edits {
                        if let TableOperations::Editing(mut edit) = edit {
                            edits_data.append(&mut edit);
                        }
                    }
                }

                history_undo.push(TableOperations::Editing(edits_data));
                history_redo.clear();
            }
            update_undo_model(self.table_model, self.undo_model);
        }
    }

    /// This function rewrite the currently selected cells using the provided formula.
    pub unsafe fn rewrite_selection(&self) {
        if let Some((is_math_operation, value)) = self.create_rewrite_selection_dialog() {

            // Get the current selection. As we need his visual order, we get it directly from the table/filter, NOT FROM THE MODEL.
            let indexes = self.table_view_primary.selection_model().selection().indexes();
            let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
            sort_indexes_visually(&mut indexes_sorted, self.table_view_primary);
            let indexes_sorted = get_real_indexes(&indexes_sorted, self.table_filter);

            let mut changed_cells = 0;
            for model_index in indexes_sorted {
                if model_index.is_valid() {

                    // Get the column of that cell, the row, the current value, and the new value.
                    let mut item = self.table_model.item_from_index(model_index.as_ref());
                    let column = model_index.column();
                    let row = model_index.row();
                    let current_value = item.text().to_std_string();
                    let new_value = value.replace("{x}", &current_value)
                        .replace("{y}", &column.to_string())
                        .replace("{z}", &row.to_string());

                    let text = if is_math_operation {
                         if let Ok(result) = meval::eval_str(&new_value) {

                            // If we got a current value and it's different, it's a valid cell.
                            match current_value.parse::<f64>() {
                                Ok(value) => {
                                    if (result - value).abs() >= std::f64::EPSILON {
                                        result.to_string()
                                    } else {
                                        current_value.to_owned()
                                    }
                                },
                                Err(_) => result.to_string(),
                            }
                        }

                        // If meval fails, it's not a valid operation for this cell
                        else { continue; }
                    } else { new_value.to_owned() };

                    let field_type = self.get_ref_table_definition().get_fields_processed()[column as usize].get_field_type();

                    // Depending on the column, we try to encode the data in one format or another.
                    match field_type {
                        FieldType::Boolean => {
                            let current_value = item.check_state();
                            let new_value = if text.to_lowercase() == "true" || text == "1" { CheckState::Checked } else { CheckState::Unchecked };
                            if current_value != new_value {
                                item.set_check_state(new_value);
                                changed_cells += 1;
                            }
                        },

                        FieldType::F32 => {
                            if current_value != text {
                                if let Ok(value) = text.parse::<f32>() {
                                    item.set_data_2a(&QVariant::from_float(value), 2);
                                    changed_cells += 1;
                                }
                            }
                        },

                        FieldType::I16 => {
                            if current_value != text {
                                if let Ok(value) = text.parse::<i16>() {
                                    item.set_data_2a(&QVariant::from_int(value.into()), 2);
                                    changed_cells += 1;
                                }
                            }
                        },

                        FieldType::I32 => {
                            if current_value != text {
                                if let Ok(value) = text.parse::<i32>() {
                                    item.set_data_2a(&QVariant::from_int(value), 2);
                                    changed_cells += 1;
                                }
                            }
                        },

                        FieldType::I64 => {
                            if current_value != text {
                                if let Ok(value) = text.parse::<i64>() {
                                    item.set_data_2a(&QVariant::from_i64(value), 2);
                                    changed_cells += 1;
                                }
                            }
                        },

                        // Skip sequences while rewriting.
                        FieldType::SequenceU16(_) |
                        FieldType::SequenceU32(_) => continue,

                        _ => {
                            if current_value != text {
                                item.set_text(&QString::from_std_str(&text));
                                changed_cells += 1;
                            }
                        }
                    }
                }
            }

            // Fix the undo history to have all the previous changed merged into one.
            if changed_cells > 0 {
                {
                    let mut history_undo = self.history_undo.write().unwrap();
                    let mut history_redo = self.history_redo.write().unwrap();

                    let len = history_undo.len();
                    let mut edits_data = vec![];
                    {
                        let mut edits = history_undo.drain((len - changed_cells)..);
                        for edit in &mut edits {
                            if let TableOperations::Editing(mut edit) = edit {
                                edits_data.append(&mut edit);
                            }
                        }
                    }

                    history_undo.push(TableOperations::Editing(edits_data));
                    history_redo.clear();
                }
                update_undo_model(self.table_model, self.undo_model);
                //undo_redo_enabler.trigger();
            }
        }
    }

    /// This function copies the selected cells into the clipboard as a TSV file, so you can paste them in other programs.
    pub unsafe fn copy_selection(&self) {

        // Get the current selection. As we need his visual order, we get it directly from the table/filter, NOT FROM THE MODEL.
        let indexes = self.table_view_primary.selection_model().selection().indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_visually(&mut indexes_sorted, self.table_view_primary);
        let indexes_sorted = get_real_indexes(&indexes_sorted, self.table_filter);

        // Create a string to keep all the values in a TSV format (x\tx\tx) and populate it.
        let mut copy = String::new();
        let mut row = 0;
        for (cycle, model_index) in indexes_sorted.iter().enumerate() {
            if model_index.is_valid() {

                // If this is the first time we loop, get the row. Otherwise, Replace the last \t with a \n and update the row.
                if cycle == 0 { row = model_index.row(); }
                else if model_index.row() != row {
                    copy.pop();
                    copy.push('\n');
                    row = model_index.row();
                }

                // If it's checkable, we need to get a bool. Otherwise it's a String.
                let item = self.table_model.item_from_index(model_index);
                if item.is_checkable() {
                    match item.check_state() {
                        CheckState::Checked => copy.push_str("true"),
                        CheckState::Unchecked => copy.push_str("false"),
                        _ => return
                    }
                }
                else { copy.push_str(&QString::to_std_string(&item.text())); }

                // Add a \t to separate fields except if it's the last field.
                if cycle < (indexes_sorted.len() - 1) { copy.push('\t'); }
            }
        }

        // Put the baby into the oven.
        QGuiApplication::clipboard().set_text_1a(&QString::from_std_str(copy));
    }

    /// This function copies the selected cells into the clipboard as a LUA Table, so you can use it in LUA scripts.
    pub unsafe fn copy_selection_as_lua_table(&self) {

        // Get the selection sorted visually.
        let indexes = self.table_view_primary.selection_model().selection().indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_visually(&mut indexes_sorted, self.table_view_primary);
        let indexes_sorted = get_real_indexes(&indexes_sorted, self.table_filter);

        // Split the indexes in two groups: those who have a key column selected and those who haven't.
        // Keep in mind this doesn't check what key column we have selected.
        //
        // TODO: Improve this.
        let (intexed_keys, indexes_no_keys): (Vec<Ref<QModelIndex>>, Vec<Ref<QModelIndex>>) = indexes_sorted.iter()
            .map(|x| x.as_ref())
            .partition(|x|
                indexes_sorted.iter()
                    .filter(|y| y.row() == x.row())
                    .any(|z| self.get_ref_table_definition().get_fields_processed()[z.column() as usize].get_is_key())
            );

        let mut lua_table = self.get_indexes_as_lua_table(&intexed_keys, true);
        lua_table.push('\n');
        lua_table.push_str(&self.get_indexes_as_lua_table(&indexes_no_keys, false));

        // Put the baby into the oven.
        QGuiApplication::clipboard().set_text_1a(&QString::from_std_str(lua_table));
    }

    /// This function allow us to paste the contents of the clipboard into the selected cells, if the content is compatible with them.
    ///
    /// This function has some... tricky stuff:
    /// - There are several special behaviors when pasting, in order to provide an Excel-Like pasting experience.
    pub unsafe fn paste(&mut self) {

        // Get the current selection. We treat it like a TSV, for compatibility with table editors.
        // Also, if the text ends in \n, remove it. Excel things.
        let mut text = QGuiApplication::clipboard().text().to_std_string();
        if text.ends_with('\n') { text.pop(); }
        let rows = text.split('\n').collect::<Vec<&str>>();
        let rows = rows.iter().map(|x| x.split('\t').collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>();

        // Get the current selection and his, visually speaking, first item (top-left).
        let indexes = self.table_view_primary.selection_model().selection().indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_visually(&mut indexes_sorted, self.table_view_primary);

        // If nothing is selected, got back to where you came from.
        if indexes_sorted.is_empty() { return }

        // At this point we should have the strings to paste and the selection. Now, clever pasting ahead:
        // - If the entire selection are rows of the same amount of cells and we have only one row of text with the exact same amount
        //   of items as the rows, we paste the same row in each selected row.
        // - If we only have one TSV value in the text and a ton of cells selcted, paste the same value everywhere.
        // - In any other case, pick the first selected cell, and paste the TSV using that as cell 0,0.
        let same_amount_of_cells_selected_per_row = if rows.len() == 1 {
            let mut row = -1;
            let mut items = 0;
            let mut is_valid = true;
            for index in &indexes_sorted {
                if row == -1 {
                    row = index.row();
                }

                if index.row() == row {
                    items += 1;
                } else {
                    if items < rows[0].len() {
                        is_valid = false;
                        break;
                    }
                    row = index.row();
                    items = 1
                }

                if items > rows[0].len() {
                    is_valid = false;
                    break;
                }
            }
            is_valid
        } else { false };

        // Amount of rows selected, to ensure certain behavior only triggers when we got the correct number of rows selected.
        let mut rows_selected = indexes_sorted.iter().map(|x| x.row()).collect::<Vec<i32>>();
        rows_selected.sort();
        rows_selected.dedup();

        if rows.len() == 1 && rows[0].len() == 1 {
            self.paste_one_for_all(&rows[0][0], &indexes_sorted);
        }

        else if rows.len() == 1 && same_amount_of_cells_selected_per_row && rows_selected.len() > 1 {
            self.paste_same_row_for_all(&rows[0], &indexes_sorted);
        }

        else {
            self.paste_as_it_fits(&rows, &indexes_sorted);
        }
    }

    /// This function pastes the value in the clipboard in every selected Cell.
    unsafe fn paste_one_for_all(&mut self, text: &str, indexes: &[Ref<QModelIndex>]) {
        let mut changed_cells = 0;
        self.save_lock.store(true, Ordering::SeqCst);

        for (index, model_index) in indexes.iter().enumerate() {
            let model_index = self.table_filter.map_to_source(*model_index);
            if model_index.is_valid() {

                // Get the column of that cell.
                let column = model_index.column();
                let mut item = self.table_model.item_from_index(model_index.as_ref());

                // Depending on the column, we try to encode the data in one format or another.
                let current_value = item.text().to_std_string();
                match self.get_ref_table_definition().get_fields_processed()[column as usize].get_ref_field_type() {
                    FieldType::Boolean => {
                        let current_value = item.check_state();
                        let new_value = if text.to_lowercase() == "true" || text == "1" { CheckState::Checked } else { CheckState::Unchecked };
                        if current_value != new_value {
                            item.set_check_state(new_value);
                            changed_cells += 1;
                        }
                    },

                    FieldType::F32 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<f32>() {
                                item.set_data_2a(&QVariant::from_float(value), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    FieldType::I16 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<i16>() {
                                item.set_data_2a(&QVariant::from_int(value.into()), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    FieldType::I32 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<i32>() {
                                item.set_data_2a(&QVariant::from_int(value), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    FieldType::I64 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<i64>() {
                                item.set_data_2a(&QVariant::from_i64(value), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    _ => {
                        if current_value != text {
                            item.set_text(&QString::from_std_str(&text));
                            changed_cells += 1;
                        }
                    }
                }

                // If it's the last cycle, trigger a save. That way we ensure a save it's done at the end.
                if index == indexes.len() - 1 {
                    self.undo_lock.store(true, Ordering::SeqCst);
                    self.table_model.item_from_index(&model_index).set_data_2a(&QVariant::from_int(1i32), 16);
                    self.save_lock.store(false, Ordering::SeqCst);
                    self.table_model.item_from_index(&model_index).set_data_2a(&QVariant::new(), 16);
                    self.undo_lock.store(false, Ordering::SeqCst);
                }
            }
        }

        // Fix the undo history to have all the previous changed merged into one.
        if changed_cells > 0 {
            {
                let mut history_undo = self.history_undo.write().unwrap();
                let mut history_redo = self.history_redo.write().unwrap();

                let len = history_undo.len();
                let mut edits_data = vec![];
                {
                    let mut edits = history_undo.drain((len - changed_cells)..);
                    for edit in &mut edits {
                        if let TableOperations::Editing(mut edit) = edit {
                            edits_data.append(&mut edit);
                        }
                    }
                }

                history_undo.push(TableOperations::Editing(edits_data));
                history_redo.clear();
            }
            update_undo_model(self.table_model, self.undo_model);
            //undo_redo_enabler.trigger();
        }
    }

    /// This function pastes the row in the clipboard in every selected row that has the same amount of items selected as items in the clipboard we have.
    unsafe fn paste_same_row_for_all(&mut self, text: &[&str], indexes: &[Ref<QModelIndex>]) {
        self.save_lock.store(true, Ordering::SeqCst);
        let mut changed_cells = 0;

        for (index, model_index) in indexes.iter().enumerate() {
            let text = text[index % text.len()];
            let model_index = self.table_filter.map_to_source(*model_index);
            if model_index.is_valid() {

                // Get the column of that cell.
                let column = model_index.column();
                let mut item = self.table_model.item_from_index(model_index.as_ref());

                // Depending on the column, we try to encode the data in one format or another.
                let current_value = item.text().to_std_string();
                match self.get_ref_table_definition().get_fields_processed()[column as usize].get_ref_field_type() {
                    FieldType::Boolean => {
                        let current_value = item.check_state();
                        let new_value = if text.to_lowercase() == "true" || text == "1" { CheckState::Checked } else { CheckState::Unchecked };
                        if current_value != new_value {
                            item.set_check_state(new_value);
                            changed_cells += 1;
                        }
                    },

                    FieldType::F32 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<f32>() {
                                item.set_data_2a(&QVariant::from_float(value), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    FieldType::I16 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<i16>() {
                                item.set_data_2a(&QVariant::from_int(value.into()), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    FieldType::I32 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<i32>() {
                                item.set_data_2a(&QVariant::from_int(value), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    FieldType::I64 => {
                        if current_value != text {
                            if let Ok(value) = text.parse::<i64>() {
                                item.set_data_2a(&QVariant::from_i64(value), 2);
                                changed_cells += 1;
                            }
                        }
                    },

                    _ => {
                        if current_value != text {
                            item.set_text(&QString::from_std_str(&text));
                            changed_cells += 1;
                        }
                    }
                }

                // If it's the last cycle, trigger a save. That way we ensure a save it's done at the end.
                if index == indexes.len() - 1 {
                    self.undo_lock.store(true, Ordering::SeqCst);
                    self.table_model.item_from_index(&model_index).set_data_2a(&QVariant::from_int(1i32), 16);
                    self.save_lock.store(false, Ordering::SeqCst);
                    self.table_model.item_from_index(&model_index).set_data_2a(&QVariant::new(), 16);
                    self.undo_lock.store(false, Ordering::SeqCst);
                }
            }
        }

        // Fix the undo history to have all the previous changed merged into one.
        if changed_cells > 0 {
            {
                let mut history_undo = self.history_undo.write().unwrap();
                let mut history_redo = self.history_redo.write().unwrap();

                let len = history_undo.len();
                let mut edits_data = vec![];
                {
                    let mut edits = history_undo.drain((len - changed_cells)..);
                    for edit in &mut edits {
                        if let TableOperations::Editing(mut edit) = edit {
                            edits_data.append(&mut edit);
                        }
                    }
                }

                history_undo.push(TableOperations::Editing(edits_data));
                history_redo.clear();
            }
            update_undo_model(self.table_model, self.undo_model);
            //undo_redo_enabler.trigger();
        }
    }

    /// This function pastes the provided text into the table as it fits, following a square strategy starting in the first selected index.
    unsafe fn paste_as_it_fits(&mut self, text: &[Vec<&str>], indexes: &[Ref<QModelIndex>]) {

        // Get the base index of the square, or stop if there is none.
        let base_index_visual = if !indexes.is_empty() {
            &indexes[0]
        } else { return };

        // We're going to try and check in square mode. That means, start in the selected cell, then right
        // until we reach a \n, then return to the initial column. Due to how sorting works, we have to do
        // a test pass first and get all the real AND VALID indexes, then try to paste on them.
        let horizontal_header = self.table_view_primary.horizontal_header();
        let vertical_header = self.table_view_primary.vertical_header();
        let mut visual_row = vertical_header.visual_index(base_index_visual.row());

        let mut real_cells = vec![];
        let mut added_rows = 0;
        for row in text {
            let mut visual_column = horizontal_header.visual_index(base_index_visual.column());
            for text in row {

                // Depending on the column, we try to encode the data in one format or another, or we just skip it.
                let real_column = horizontal_header.logical_index(visual_column);
                let mut real_row = vertical_header.logical_index(visual_row);
                let definition = self.get_ref_table_definition().clone();
                if let Some(field) = definition.get_fields_processed().get(real_column as usize) {

                    // Check if, according to the definition, we have a valid value for the type.
                    let is_valid_data = match field.get_ref_field_type() {
                        FieldType::Boolean => !(text.to_lowercase() != "true" && text.to_lowercase() != "false" && text != &"1" && text != &"0"),
                        FieldType::F32 => text.parse::<f32>().is_ok(),
                        FieldType::I16 => text.parse::<i16>().is_ok(),
                        FieldType::I32 => text.parse::<i32>().is_ok(),
                        FieldType::I64 => text.parse::<i64>().is_ok(),

                        // All these are Strings, so we can skip their checks....
                        FieldType::StringU8 |
                        FieldType::StringU16 |
                        FieldType::OptionalStringU8 |
                        FieldType::OptionalStringU16 => true,

                        // Ignore sequences.
                        FieldType::SequenceU16(_) | FieldType::SequenceU32(_) => false,
                    };

                    // If it's valid, add it to the real_cells list.
                    if is_valid_data {

                        // If real_row is -1 (invalid), then we need to add an empty row to the model (NOT TO THE FILTER)
                        // because that means we have no row for that position, and we need one.
                        if real_row == -1 {
                            let row = get_new_row(&self.get_ref_table_definition());
                            self.table_model.append_row_q_list_of_q_standard_item(&row);
                            real_row = self.table_model.row_count_0a() - 1;
                            added_rows += 1;
                        }
                        real_cells.push((self.table_filter.map_to_source(&self.table_filter.index_2a(real_row, real_column)), text));
                    }
                }
                visual_column += 1;
            }
            visual_row += 1;
        }

        // We need to update the undo model here, because otherwise it'll start triggering crashes
        // in case the first thing to paste is equal to the current value. In that case, the set_data
        // will not trigger, and the update_undo_model will not trigger either, causing a crash if
        // inmediatly after that we try to paste something in a new line (which will not exist in the undo model).
        {
            //let mut table_state_data = table_state_data.borrow_mut();
            //let table_state_data = table_state_data.get_mut(&*packed_file_path.borrow()).unwrap();
            update_undo_model(self.table_model, self.undo_model);
        }

        self.save_lock.store(true, Ordering::SeqCst);

        // Now we do the real pass, changing data if needed.
        let mut changed_cells = 0;
        for (index, (real_cell, text)) in real_cells.iter().enumerate() {

            // Depending on the column, we try to encode the data in one format or another.
            let current_value = self.table_model.data_1a(real_cell).to_string().to_std_string();
            let definition = self.get_ref_table_definition().clone();
            match definition.get_fields_processed()[real_cell.column() as usize].get_ref_field_type() {

                FieldType::Boolean => {
                    let current_value = self.table_model.item_from_index(real_cell).check_state();
                    let new_value = if text.to_lowercase() == "true" || **text == "1" { CheckState::Checked } else { CheckState::Unchecked };
                    if current_value != new_value {
                        self.table_model.item_from_index(real_cell).set_check_state(new_value);
                        changed_cells += 1;
                    }
                },

                FieldType::F32 => {
                    if &current_value != *text {
                        self.table_model.set_data_3a(real_cell, &QVariant::from_float(text.parse::<f32>().unwrap()), 2);
                        changed_cells += 1;
                    }
                },

                FieldType::I16 => {
                    if &current_value != *text {
                        self.table_model.set_data_3a(real_cell, &QVariant::from_int(text.parse::<i16>().unwrap() as i32), 2);
                        changed_cells += 1;
                    }
                },

                FieldType::I32 => {
                    if &current_value != *text {
                        self.table_model.set_data_3a(real_cell, &QVariant::from_int(text.parse::<i32>().unwrap()), 2);
                        changed_cells += 1;
                    }
                },

                FieldType::I64 => {
                    if &current_value != *text {
                        self.table_model.set_data_3a(real_cell, &QVariant::from_i64(text.parse::<i64>().unwrap()), 2);
                        changed_cells += 1;
                    }
                },

                _ => {
                    if &current_value != *text {
                        self.table_model.set_data_3a(real_cell, &QVariant::from_q_string(&QString::from_std_str(text)), 2);
                        changed_cells += 1;
                    }
                }
            }

            // If it's the last cycle, trigger a save. That way we ensure a save it's done at the end.
            if index == real_cells.len() - 1 {
                self.undo_lock.store(true, Ordering::SeqCst);
                self.table_model.item_from_index(real_cell).set_data_2a(&QVariant::from_int(1i32), 16);
                self.save_lock.store(false, Ordering::SeqCst);
                self.table_model.item_from_index(real_cell).set_data_2a(&QVariant::new(), 16);
                self.undo_lock.store(false, Ordering::SeqCst);
            }
        }

        // Fix the undo history to have all the previous changed merged into one. Or that's what I wanted.
        // Sadly, the world doesn't work like that. As we can edit AND add rows, we have to use a combined undo operation.
        // I'll call it... Carolina.
        if changed_cells > 0 || added_rows > 0 {
            {
                let mut history_undo = self.history_undo.write().unwrap();
                let mut history_redo = self.history_redo.write().unwrap();

                let len = history_undo.len();
                let mut carolina = vec![];
                if changed_cells > 0 {

                    let mut edits_data = vec![];
                    let mut edits = history_undo.drain((len - changed_cells)..);
                    for edit in &mut edits {
                        if let TableOperations::Editing(mut edit) = edit {
                            edits_data.append(&mut edit);
                        }
                    }
                    carolina.push(TableOperations::Editing(edits_data));
                }

                if added_rows > 0 {
                    let mut rows = vec![];
                    ((self.table_model.row_count_0a() - added_rows)..self.table_model.row_count_0a()).rev().for_each(|x| rows.push(x));
                    carolina.push(TableOperations::AddRows(rows));
                }

                history_undo.push(TableOperations::Carolina(carolina));
                history_redo.clear();
            }

            update_undo_model(self.table_model, self.undo_model);
            //unsafe { undo_redo_enabler.as_mut().unwrap().trigger(); }
        }
    }

    /// Function to undo/redo an operation in the table.
    ///
    /// If undo = true we are undoing. Otherwise we are redoing.
    /// NOTE: repeat_x_times is for internal recursion!!! ALWAYS PUT A 0 THERE!!!.
    pub unsafe fn undo_redo(
        &mut self,
        undo: bool,
        mut repeat_x_times: usize,
    ) {
        let filter: MutPtr<QSortFilterProxyModel> = self.table_view_primary.model().static_downcast_mut();
        let mut model: MutPtr<QStandardItemModel> = filter.source_model().static_downcast_mut();
        let mut is_carolina = false;

        {
            let (mut history_source, mut history_opposite) = if undo {
                (self.history_undo.write().unwrap(), self.history_redo.write().unwrap())
            } else {
                (self.history_redo.write().unwrap(), self.history_undo.write().unwrap())
            };

            // Get the last operation in the Undo History, or return if there is none.
            let operation = if let Some(operation) = history_source.pop() { operation } else { return };
            log_to_status_bar(&format!("{:?}", operation));
            match operation {
                TableOperations::Editing(editions) => {

                    // Prepare the redo operation, then do the rest.
                    let mut redo_editions = vec![];
                    editions.iter().for_each(|x| redo_editions.push((((x.0).0, (x.0).1), atomic_from_mut_ptr((&*model.item_2a((x.0).0, (x.0).1)).clone()))));
                    history_opposite.push(TableOperations::Editing(redo_editions));

                    self.undo_lock.store(true, Ordering::SeqCst);
                    for (index, ((row, column), item)) in editions.iter().enumerate() {
                        let item = &*mut_ptr_from_atomic(&item);
                        model.set_item_3a(*row, *column, item.clone());

                        // If we are going to process the last one, unlock the save.
                        if index == editions.len() - 1 {
                            model.item_2a(*row, *column).set_data_2a(&QVariant::from_int(1i32), 16);
                            model.item_2a(*row, *column).set_data_2a(&QVariant::new(), 16);
                        }
                    }

                    // Select all the edited items.
                    let mut selection_model = self.table_view_primary.selection_model();
                    selection_model.clear();

                    // TODO: This is still very slow. We need some kind of range optimization.
                    let _blocker = QSignalBlocker::from_q_object(selection_model);
                    for ((row, column),_) in &editions {
                        let model_index_filtered = filter.map_from_source(&model.index_2a(*row, *column));
                        if model_index_filtered.is_valid() {
                            selection_model.select_q_model_index_q_flags_selection_flag(
                                &model_index_filtered,
                                QFlags::from(SelectionFlag::Select)
                            );
                        }
                    }

                    self.undo_lock.store(false, Ordering::SeqCst);
                }

                // This actions if for undoing "add rows" actions. It deletes the stored rows.
                TableOperations::AddRows(mut rows) => {

                    // Sort them 0->9, so we can process them.
                    rows.sort_by(|x, y| x.cmp(y));
                    self.undo_lock.store(true, Ordering::SeqCst);
                    let rows_splitted = delete_rows(self.table_model, &rows);
                    history_opposite.push(TableOperations::RemoveRows(rows_splitted));
                    self.undo_lock.store(false, Ordering::SeqCst);
                }

                // NOTE: the rows list must ALWAYS be in 1->9 order. Otherwise this breaks.
                TableOperations::RemoveRows(mut rows) => {
                    self.undo_lock.store(true, Ordering::SeqCst);
                    self.save_lock.store(true, Ordering::SeqCst);

                    // Make sure the order of these ones is always correct (9->0).
                    rows.sort_by(|x, y| x.0.cmp(&y.0));

                    // First, we re-create the rows and re-insert them.
                    for (index, row_pack) in &rows {
                        for (offset, row) in row_pack.iter().enumerate() {
                            let mut qlist = QListOfQStandardItem::new();
                            row.iter().for_each(|x| add_to_q_list_safe(qlist.as_mut_ptr(), mut_ptr_from_atomic(x)));
                            model.insert_row_int_q_list_of_q_standard_item(*index + offset as i32, &qlist);
                        }
                    }

                    // Then, create the redo action for this one.
                    let mut rows_to_add = rows.iter()
                        .map(|(index, row_pack)|
                            row_pack.iter().enumerate()
                                .map(|(x, _)| *index + x as i32)
                                .collect::<Vec<i32>>()
                        )
                        .flatten()
                        .collect::<Vec<i32>>();

                    rows_to_add.reverse();
                    history_opposite.push(TableOperations::AddRows(rows_to_add));

                    // Select all the re-inserted rows that are in the filter. We need to block signals here because the bigger this gets,
                    // the slower it gets. And it gets very slow on high amounts of lines.
                    let mut selection_model = self.table_view_primary.selection_model();
                    selection_model.clear();
                    for (index, row_pack) in &rows {
                        let initial_model_index_filtered = self.table_filter.map_from_source(&self.table_model.index_2a(*index, 0));
                        let final_model_index_filtered = self.table_filter.map_from_source(&self.table_model.index_2a(*index + row_pack.len() as i32 - 1, 0));
                        if initial_model_index_filtered.is_valid() && final_model_index_filtered.is_valid() {
                            let selection = QItemSelection::new_2a(&initial_model_index_filtered, &final_model_index_filtered);
                            selection_model.select_q_item_selection_q_flags_selection_flag(&selection, SelectionFlag::Select | SelectionFlag::Rows);
                        }
                    }

                    // Trick to tell the model to update everything.
                    self.save_lock.store(false, Ordering::SeqCst);
                    model.item_2a(0, 0).set_data_2a(&QVariant::new(), 16);
                    self.undo_lock.store(false, Ordering::SeqCst);
                }

                // This action is special and we have to manually trigger a save for it.
                TableOperations::ImportTSV(table_data) => {

                    let old_data = self.get_copy_of_table();
                    history_opposite.push(TableOperations::ImportTSV(old_data));

                    let row_count = self.table_model.row_count_0a();
                    self.table_model.remove_rows_2a(0, row_count);
                    for row in &table_data {
                        let row = mut_ptr_from_atomic(row);
                        self.table_model.append_row_q_list_of_q_standard_item(row.as_ref().unwrap())
                    }
                }

                TableOperations::Carolina(mut operations) => {
                    is_carolina = true;
                    repeat_x_times = operations.len();
                    operations.reverse();
                    history_source.append(&mut operations);
                }
            }

            // We have to manually update these from the context menu due to RwLock deadlocks.
            if undo {
                self.context_menu_undo.set_enabled(!history_source.is_empty());
                self.context_menu_redo.set_enabled(!history_opposite.is_empty());
            }
            else {
                self.context_menu_redo.set_enabled(!history_source.is_empty());
                self.context_menu_undo.set_enabled(!history_opposite.is_empty());
            }
        }

        // If we have repetitions, it means we got a carolina. Repeat all the times we need until all editions are undone.
        // Then, remove all the actions done and put them into a carolina.
        if repeat_x_times > 0 {
            self.undo_redo(undo, repeat_x_times - 1);
            if is_carolina {
                let mut history_opposite = if undo {
                    self.history_redo.write().unwrap()
                } else {
                    self.history_undo.write().unwrap()
                };
                let len = history_opposite.len();
                let mut edits = history_opposite.drain((len - repeat_x_times)..).collect::<Vec<TableOperations>>();
                edits.reverse();
                history_opposite.push(TableOperations::Carolina(edits));
            }
        }
    }

    /// This function returns the provided indexes's data as a LUA table.
    unsafe fn get_indexes_as_lua_table(&self, indexes: &[Ref<QModelIndex>], has_keys: bool) -> String {
        let mut table_data: Vec<(Option<String>, Vec<String>)> = vec![];
        let mut last_row = None;
        for index in indexes {
            let current_row = index.row();
            match last_row {
                Some(row) => {

                    // If it's the same row as before, take the row from the table data and append it.
                    if current_row == row {
                        let entry = table_data.last_mut().unwrap();
                        let data = self.get_escaped_lua_string_from_index(*index);
                        if entry.0.is_none() && self.get_ref_table_definition().get_fields_processed()[index.column() as usize].get_is_key() {
                            entry.0 = Some(self.escape_string_from_index(*index));
                        }
                        entry.1.push(data);
                    }

                    // If it's not the same row as before, we create it as a new row.
                    else {
                        let mut entry = (None, vec![]);
                        let data = self.get_escaped_lua_string_from_index(*index);
                        entry.1.push(data.to_string());
                        if entry.0.is_none() && self.get_ref_table_definition().get_fields_processed()[index.column() as usize].get_is_key() {
                            entry.0 = Some(self.escape_string_from_index(*index));
                        }
                        table_data.push(entry);
                    }
                }
                None => {
                    let mut entry = (None, vec![]);
                    let data = self.get_escaped_lua_string_from_index(*index);
                    entry.1.push(data.to_string());
                    if entry.0.is_none() && self.get_ref_table_definition().get_fields_processed()[index.column() as usize].get_is_key() {
                        entry.0 = Some(self.escape_string_from_index(*index));
                    }
                    table_data.push(entry);
                }
            }

            last_row = Some(current_row);
        }

        // Create the string of the table.
        let mut lua_table = String::new();

        if !table_data.is_empty() {
            if has_keys {
                lua_table.push_str("TABLE = {\n");
            }

            for (index, row) in table_data.iter().enumerate() {

                // Start the row.
                if let Some(key) = &row.0 {
                    lua_table.push_str(&format!("\t[{}] = {{", key));
                }
                else {
                    lua_table.push('{');
                }

                // For each cell in the row, push it to the LUA Table.
                for cell in row.1.iter() {
                    lua_table.push_str(cell);
                }

                // Take out the last comma.
                lua_table.pop();

                // Close the row.
                if index == row.1.len() - 1 {
                    lua_table.push_str(" }\n");
                }
                else {
                    lua_table.push_str(" },\n");
                }
            }

            if has_keys {
                lua_table.push_str("}");
            }
        }

        lua_table
    }

    /// This function turns the data from the provided indexes into LUA compatible strings.
    unsafe fn get_escaped_lua_string_from_index(&self, index: Ref<QModelIndex>) -> String {
        format!(" [\"{}\"] = {},", self.get_ref_table_definition().get_fields_processed()[index.column() as usize].get_name(), self.escape_string_from_index(index))
    }

    /// This function escapes the value inside an index.
    unsafe fn escape_string_from_index(&self, index: Ref<QModelIndex>) -> String {
        let item = self.table_model.item_from_index(index);
        let definition = &self.get_ref_table_definition().clone();
        match definition.get_fields_processed()[index.column() as usize].get_ref_field_type() {
            FieldType::Boolean => if let CheckState::Checked = item.check_state() { "true".to_owned() } else { "false".to_owned() },

            // Floats need to be tweaked to fix trailing zeroes and precission issues, like turning 0.5000004 into 0.5.
            FieldType::F32 => {
                let data_str = format!("{}", item.data_1a(2).to_float_0a());

                // If we have more than 3 decimals, we limit it to three, then do magic to remove trailing zeroes.
                if let Some(position) = data_str.find('.') {
                    let decimals = &data_str[position..].len();
                    if *decimals > 3 { format!("{}", format!("{:.3}", item.data_1a(2).to_float_0a()).parse::<f32>().unwrap()) }
                    else { data_str }
                }
                else { data_str }
            },
            FieldType::I16 |
            FieldType::I32 |
            FieldType::I64 => format!("{}", item.data_1a(2).to_long_long_0a()),

            // All these are Strings, so they need to escape certain chars and include commas in Lua.
            FieldType::StringU8 |
            FieldType::StringU16 |
            FieldType::OptionalStringU8 |
            FieldType::OptionalStringU16 => format!("\"{}\"", item.text().to_std_string().escape_default().to_string()),
            FieldType::SequenceU16(_) => "\"SequenceU16\"".to_owned(),
            FieldType::SequenceU32(_) => "\"SequenceU32\"".to_owned(),
        }
    }

    /// This function is used to append new rows to a table.
    ///
    /// If clone = true, the appended rows are copies of the selected ones.
    pub unsafe fn append_rows(&mut self, clone: bool) {

        // Get the indexes ready for battle.
        let selection = self.table_view_primary.selection_model().selection();
        let indexes = self.table_filter.map_selection_to_source(&selection).indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_by_model(&mut indexes_sorted);
        dedup_indexes_per_row(&mut indexes_sorted);
        let mut row_numbers = vec![];

        let rows = if clone {
            let mut rows = vec![];
            let color = get_color_added_modified();
            for index in indexes_sorted.iter() {
                row_numbers.push(index.row());

                let columns = self.table_model.column_count_0a();
                let mut qlist = QListOfQStandardItem::new();
                for column in 0..columns {
                    let original_item = self.table_model.item_2a(index.row(), column);
                    let mut item = (*original_item).clone();
                    item.set_background(&QBrush::from_q_color(color.as_ref().unwrap()));
                    add_to_q_list_safe(qlist.as_mut_ptr(), item);
                }

                rows.push(qlist);
            }
            rows
        } else {
            let color = get_color_added();
            let mut row = get_new_row(&self.get_ref_table_definition());
            for index in 0..row.count() {
                row.index(index).as_mut().unwrap().set_background(&QBrush::from_q_color(color.as_ref().unwrap()));
            }
            vec![row]
        };

        let mut selection_model = self.table_view_primary.selection_model();
        selection_model.clear();
        for row in &rows {
            self.table_model.append_row_q_list_of_q_standard_item(row.as_ref());

            // Select the row and scroll to it.
            let model_index_filtered = self.table_filter.map_from_source(&self.table_model.index_2a(self.table_filter.row_count_0a() - 1, 0));
            if model_index_filtered.is_valid() {
                selection_model.select_q_model_index_q_flags_selection_flag(
                    &model_index_filtered,
                    SelectionFlag::Select | SelectionFlag::Rows
                );

                self.table_view_primary.scroll_to_2a(
                    model_index_filtered.as_ref(),
                    ScrollHint::EnsureVisible
                );
            }
        }

        // Update the undo stuff. Cloned rows are the amount of rows - the amount of cloned rows.
        let total_rows = self.table_model.row_count_0a();
        let range = (total_rows - rows.len() as i32..total_rows).collect::<Vec<i32>>();
        self.history_undo.write().unwrap().push(TableOperations::AddRows(range));
        self.history_redo.write().unwrap().clear();
        update_undo_model(self.table_model, self.undo_model);
        //unsafe { undo_redo_enabler.as_mut().unwrap().trigger(); }
    }

    /// This function is used to insert new rows into a table.
    ///
    /// If clone = true, the appended rows are copies of the selected ones.
    pub unsafe fn insert_rows(&mut self, clone: bool) {

        // Get the indexes ready for battle.
        let selection = self.table_view_primary.selection_model().selection();
        let indexes = self.table_filter.map_selection_to_source(&selection).indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_by_model(&mut indexes_sorted);
        dedup_indexes_per_row(&mut indexes_sorted);
        let mut row_numbers = vec![];

        // If nothing is selected, we just append one new row at the end. This only happens when adding empty rows, so...
        if indexes_sorted.is_empty() {
            let row = get_new_row(&self.get_ref_table_definition());
            self.table_model.append_row_q_list_of_q_standard_item(&row);
            row_numbers.push(self.table_model.row_count_0a() - 1);
        }

        let mut selection_model = self.table_view_primary.selection_model();
        selection_model.clear();

        for index in indexes_sorted.iter().rev() {
            row_numbers.push(index.row() + (indexes_sorted.len() - row_numbers.len() - 1) as i32);

            // If we want to clone, we copy the currently selected row. If not, we just create a new one.
            let row = if clone {
                let color = get_color_added_modified();
                let columns = self.table_model.column_count_0a();
                let mut qlist = QListOfQStandardItem::new();
                for column in 0..columns {
                    let original_item = self.table_model.item_2a(index.row(), column);
                    let mut item = (*original_item).clone();
                    item.set_background(&QBrush::from_q_color(color.as_ref().unwrap()));
                    add_to_q_list_safe(qlist.as_mut_ptr(), item);
                }
                qlist
            } else {
                let color = get_color_added();
                let mut row = get_new_row(&self.get_ref_table_definition());
                for index in 0..row.count() {
                    row.index(index).as_mut().unwrap().set_background(&QBrush::from_q_color(color.as_ref().unwrap()));
                }
                row
            };
            self.table_model.insert_row_int_q_list_of_q_standard_item(index.row(), &row);

            // Select the row.
            let model_index_filtered = self.table_filter.map_from_source(&self.table_model.index_2a(index.row(), 0));
            if model_index_filtered.is_valid() {
                selection_model.select_q_model_index_q_flags_selection_flag(
                    &model_index_filtered,
                    SelectionFlag::Select | SelectionFlag::Rows
                );
            }
        }

        // The undo mode needs this reversed.
        self.history_undo.write().unwrap().push(TableOperations::AddRows(row_numbers));
        self.history_redo.write().unwrap().clear();
        update_undo_model(self.table_model, self.undo_model);
    }

    /// This function returns a copy of the entire model.
    pub unsafe fn get_copy_of_table(&self) -> Vec<AtomicPtr<QListOfQStandardItem>> {
        let mut old_data = vec![];
        for row in 0..self.table_model.row_count_0a() {
            let mut qlist = QListOfQStandardItem::new();
            for column in 0..self.table_model.column_count_0a() {
                let item = self.table_model.item_2a(row, column);
                add_to_q_list_safe(qlist.as_mut_ptr(), (*item).clone());
            }
            old_data.push(atomic_from_mut_ptr(qlist.into_ptr()));
        }
        old_data
    }

    /// This function creates the entire "Rewrite selection" dialog for tables. It returns the rewriting sequence, or None.
    pub unsafe fn create_rewrite_selection_dialog(&self) -> Option<(bool, String)> {

        // Create and configure the dialog.
        let mut dialog = QDialog::new_1a(self.table_view_primary);
        dialog.set_window_title(&qtr("rewrite_selection_title"));
        dialog.set_modal(true);
        dialog.resize_2a(400, 50);
        let mut main_grid = create_grid_layout(dialog.as_mut_ptr().static_upcast_mut());

        // Create a little frame with some instructions.
        let instructions_frame = QGroupBox::from_q_string(&qtr("rewrite_selection_instructions_title")).into_ptr();
        let mut instructions_grid = create_grid_layout(instructions_frame.static_upcast_mut());
        let mut instructions_label = QLabel::from_q_string(&qtr("rewrite_selection_instructions"));
        instructions_grid.add_widget_5a(&mut instructions_label, 0, 0, 1, 1);

        let mut is_math_op = QCheckBox::from_q_string(&qtr("rewrite_selection_is_math"));
        let mut rewrite_sequence_line_edit = QLineEdit::new();
        rewrite_sequence_line_edit.set_placeholder_text(&qtr("rewrite_selection_placeholder"));
        let mut accept_button = QPushButton::from_q_string(&qtr("rewrite_selection_accept"));

        main_grid.add_widget_5a(instructions_frame, 0, 0, 1, 2);
        main_grid.add_widget_5a(&mut is_math_op, 1, 0, 1, 2);
        main_grid.add_widget_5a(&mut rewrite_sequence_line_edit, 2, 0, 1, 1);
        main_grid.add_widget_5a(&mut accept_button, 2, 1, 1, 1);

        accept_button.released().connect(dialog.slot_accept());

        if dialog.exec() == 1 {
            let new_text = rewrite_sequence_line_edit.text().to_std_string();
            if new_text.is_empty() { None } else { Some((is_math_op.is_checked(), rewrite_sequence_line_edit.text().to_std_string())) }
        } else { None }
    }

    /// This function takes care of the "Smart Delete" feature for tables.
    pub unsafe fn smart_delete(&mut self) {

        // Get the selected indexes, the split them in two groups: one with full rows selected and another with single cells selected.
        let indexes = self.table_view_primary.selection_model().selection().indexes();
        let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
        sort_indexes_visually(&mut indexes_sorted, self.table_view_primary);
        let indexes_sorted = get_real_indexes(&indexes_sorted, self.table_filter);

        let mut cells: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        for model_index in &indexes_sorted {
            if model_index.is_valid() {
                let row = model_index.row();
                let column = model_index.column();

                // Check if we have any cell in that row and add/insert the new one.
                match cells.get_mut(&row) {
                    Some(row) => row.push(column),
                    None => { cells.insert(row, vec![column]); },
                }
            }
        }

        let full_rows = cells.iter()
            .filter(|(_, y)| y.len() as i32 == self.table_model.column_count_0a())
            .map(|(x, _)| *x)
            .collect::<Vec<i32>>();

        let individual_cells = cells.iter()
            .filter(|(_, y)| y.len() as i32 != self.table_model.column_count_0a())
            .map(|(x, y)| (*x, y.to_vec()))
            .collect::<Vec<(i32, Vec<i32>)>>();

        // First, we do the editions. This means:
        // - Checkboxes: unchecked.
        // - Numbers: 0.
        // - Strings: empty.
        let mut editions = 0;
        for (row, columns) in &individual_cells {
            for column in columns {
                let mut item = self.table_model.item_2a(*row, *column);
                let current_value = item.text().to_std_string();
                match self.get_ref_table_definition().get_fields_processed()[*column as usize].get_ref_field_type() {
                    FieldType::Boolean => {
                        let current_value = item.check_state();
                        if current_value != CheckState::Unchecked {
                            item.set_check_state(CheckState::Unchecked);
                            editions += 1;
                        }
                    }

                    FieldType::F32 => {
                        if !current_value.is_empty() {
                            item.set_data_2a(&QVariant::from_float(0.0f32), 2);
                            editions += 1;
                        }
                    }

                    FieldType::I16 => {
                        if !current_value.is_empty() {
                            item.set_data_2a(&QVariant::from_int(0i32), 2);
                            editions += 1;
                        }
                    }

                    FieldType::I32 => {
                        if !current_value.is_empty() {
                            item.set_data_2a(&QVariant::from_int(0i32), 2);
                            editions += 1;
                        }
                    }

                    FieldType::I64 => {
                        if !current_value.is_empty() {
                            item.set_data_2a(&QVariant::from_i64(0i64), 2);
                            editions += 1;
                        }
                    }

                    _ => {
                        if !current_value.is_empty() {
                            item.set_text(&QString::from_std_str(""));
                            editions += 1;
                        }
                    }
                }
            }
        }

        // Then, we delete all the fully selected rows.
        let rows_splitted = super::utils::delete_rows(self.table_model, &full_rows);

        // Then, we have to fix the undo history. For that, we take out all the editions, merge them,
        // then merge them with the table edition into a carolina.
        if editions > 0 || !rows_splitted.is_empty() {

            // Update the search stuff, if needed.
            //unsafe { update_search_stuff.as_mut().unwrap().trigger(); }

             {
                let mut changes = vec![];
                if !rows_splitted.is_empty() {
                    changes.push(TableOperations::RemoveRows(rows_splitted));
                }

                let len = self.history_undo.read().unwrap().len();
                let editions: Vec<((i32, i32), AtomicPtr<QStandardItem>)> = self.history_undo.write().unwrap()
                    .drain(len - editions..)
                    .filter_map(|x| if let TableOperations::Editing(y) = x { Some(y) } else { None })
                    .flatten()
                    .collect();

                if !editions.is_empty() {
                    changes.push(TableOperations::Editing(editions));
                }

                if !changes.is_empty() {
                    self.history_undo.write().unwrap().push(TableOperations::Carolina(changes));
                    self.history_redo.write().unwrap().clear();
                    update_undo_model(self.table_model, self.undo_model);
                    self.context_menu_update();
                }
            }
        }
    }
}
