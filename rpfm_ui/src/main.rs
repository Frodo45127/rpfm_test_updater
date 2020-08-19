//---------------------------------------------------------------------------//
// Copyright (c) 2017-2020 Ismael Gutiérrez González. All rights reserved.
//
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
//
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

// This is the main file of RPFM. Here is the main loop that builds the UI and controls his events.

// Disabled `Clippy` linters, with the reasons why they were disabled.
#![allow(
    clippy::cognitive_complexity,           // Disabled due to useless warnings.
    //clippy::cyclomatic_complexity,          // Disabled due to useless warnings.
    clippy::if_same_then_else,              // Disabled because some of the solutions it provides are freaking hard to read.
    clippy::match_bool,                     // Disabled because the solutions it provides are harder to read than the current code.
    clippy::new_ret_no_self,                // Disabled because the reported situations are special cases. So no, I'm not going to rewrite them.
    clippy::suspicious_else_formatting,     // Disabled because the errors it gives are actually false positives due to comments.
    clippy::match_wild_err_arm,              // Disabled because, despite being a bad practice, it's the intended behavior in the code it warns about.
    clippy::large_enum_variant,
    clippy::clone_on_copy
)]

// This disables the terminal window, so it doesn't show up when executing RPFM in Windows.
#![windows_subsystem = "windows"]

use qt_widgets::QApplication;
use qt_widgets::QStatusBar;

use qt_gui::QColor;
use qt_gui::QFont;
use qt_gui::{QPalette, q_palette::{ColorGroup, ColorRole}};
use qt_gui::QFontDatabase;
use qt_gui::q_font_database::SystemFont;

use qt_core::QString;

use lazy_static::lazy_static;
use log::info;
use simplelog::{CombinedLogger, LevelFilter, TerminalMode, TermLogger, WriteLogger};

use std::cell::RefCell;
use std::fs::File;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicPtr;
use std::thread;

use rpfm_error::ctd::CrashReport;
use rpfm_error::{Error, ErrorKind};

use rpfm_lib::config::{init_config_path, get_config_path};
use rpfm_lib::SETTINGS;

use crate::app_ui::AppUI;
use crate::communications::CentralCommand;
use crate::locale::Locale;
use crate::pack_tree::icons::Icons;
use crate::ui::GameSelectedIcons;
use crate::ui_state::UIState;
use crate::ui::UI;
use crate::utils::atomic_from_cpp_box;

/// This macro is used to clone the variables into the closures without the compiler complaining.
/// This should be BEFORE the `mod xxx` stuff, so submodules can use it too.
macro_rules! clone {
    (@param _) => ( _ );
    (@param $x:ident) => ( $x );
    ($($n:ident),+ => move || $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move || $body
        }
    );
    ($($y:ident $n:ident),+ => move || $body:expr) => (
        {
            $( #[allow(unused_mut)] let mut $n = $n.clone(); )+
            move || $body
        }
    );
    ($($n:ident),+ => move |$($p:tt),+| $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move |$(clone!(@param $p),)+| $body
        }
    );
    ($($y:ident $n:ident),+ => move |$($p:tt),+| $body:expr) => (
        {
            $( #[allow(unused_mut)] let mut $n = $n.clone(); )+
            move |$(clone!(@param $p),)+| $body
        }
    );
}

mod app_ui;
mod background_thread;
mod command_palette;
mod communications;
mod ffi;
mod global_search_ui;
mod locale;
mod mymod_ui;
mod network_thread;
mod pack_tree;
mod packfile_contents_ui;
mod packedfile_views;
mod shortcuts_ui;
mod settings_ui;
mod ui;
mod ui_state;
mod utils;
mod views;

// Statics, so we don't need to pass them everywhere to use them.
lazy_static! {

    /// Path were the stuff used by RPFM (settings, schemas,...) is. In debug mode, we just take the current path
    /// (so we don't break debug builds). In Release mode, we take the `.exe` path.
    #[derive(Debug)]
    static ref RPFM_PATH: PathBuf = if cfg!(debug_assertions) {
        std::env::current_dir().unwrap()
    } else {
        let mut path = std::env::current_exe().unwrap();
        path.pop();
        path
    };

    /// Path that contains the extra assets we need, like images.
    #[derive(Debug)]
    static ref ASSETS_PATH: PathBuf = if cfg!(debug_assertions) {
        RPFM_PATH.to_path_buf()
    } else {
        // For release builds:
        // - Windows: Same as RFPM exe.
        // - Linux: /usr/share/rpfm.
        // - MacOs: Who knows?
        if cfg!(target_os = "linux") {
            PathBuf::from("/usr/share/rpfm")
        }
        //if cfg!(target_os = "windows") {
        else {
            RPFM_PATH.to_path_buf()
        }
    };

    /// Icons for the PackFile TreeView.
    static ref TREEVIEW_ICONS: Icons = unsafe { Icons::new() };

    /// Icons for the `Game Selected` in the TitleBar.
    static ref GAME_SELECTED_ICONS: GameSelectedIcons = unsafe { GameSelectedIcons::new() };

    /// Bright and dark palettes of colours for Windows.
    /// The dark one is taken from here, with some modifications: https://gist.github.com/QuantumCD/6245215
    static ref LIGHT_PALETTE: AtomicPtr<QPalette> = unsafe { atomic_from_cpp_box(QPalette::new()) };
    static ref DARK_PALETTE: AtomicPtr<QPalette> = unsafe {{
        let mut palette = QPalette::new();

        // Base config.
        palette.set_color_2a(ColorRole::Window, &QColor::from_3_int(51, 51, 51));
        palette.set_color_2a(ColorRole::WindowText, &QColor::from_3_int(187, 187, 187));
        palette.set_color_2a(ColorRole::Base, &QColor::from_3_int(34, 34, 34));
        palette.set_color_2a(ColorRole::AlternateBase, &QColor::from_3_int(51, 51, 51));
        palette.set_color_2a(ColorRole::ToolTipBase, &QColor::from_3_int(187, 187, 187));
        palette.set_color_2a(ColorRole::ToolTipText, &QColor::from_3_int(187, 187, 187));
        palette.set_color_2a(ColorRole::Text, &QColor::from_3_int(187, 187, 187));
        palette.set_color_2a(ColorRole::Button, &QColor::from_3_int(51, 51, 51));
        palette.set_color_2a(ColorRole::ButtonText, &QColor::from_3_int(187, 187, 187));
        palette.set_color_2a(ColorRole::BrightText, &QColor::from_3_int(255, 0, 0));
        palette.set_color_2a(ColorRole::Link, &QColor::from_3_int(42, 130, 218));
        palette.set_color_2a(ColorRole::Highlight, &QColor::from_3_int(42, 130, 218));
        palette.set_color_2a(ColorRole::HighlightedText, &QColor::from_3_int(204, 204, 204));

        // Disabled config.
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::Window, &QColor::from_3_int(34, 34, 34));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::WindowText, &QColor::from_3_int(85, 85, 85));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::Base, &QColor::from_3_int(34, 34, 34));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::AlternateBase, &QColor::from_3_int(34, 34, 34));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::ToolTipBase, &QColor::from_3_int(85, 85, 85));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::ToolTipText, &QColor::from_3_int(85, 85, 85));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::Text, &QColor::from_3_int(85, 85, 85));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::Button, &QColor::from_3_int(34, 34, 34));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::ButtonText, &QColor::from_3_int(85, 85, 85));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::BrightText, &QColor::from_3_int(170, 0, 0));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::Link, &QColor::from_3_int(42, 130, 218));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::Highlight, &QColor::from_3_int(42, 130, 218));
        palette.set_color_3a(ColorGroup::Disabled, ColorRole::HighlightedText, &QColor::from_3_int(85, 85, 85));

        atomic_from_cpp_box(palette)
    }};

    /// Stylesheet used by the dark theme in Windows.
    static ref DARK_STYLESHEET: String = utils::create_dark_theme_stylesheet();

    // Colors used all over the program for theming and stuff.
    static ref MEDIUM_DARK_GREY: &'static str = "#333333";            // Medium-Dark Grey. The color of the background of the Main Window.
    static ref MEDIUM_DARKER_GREY: &'static str = "#262626";          // Medium-Darker Grey.
    static ref DARK_GREY: &'static str = "#181818";                   // Dark Grey. The color of the background of the Main TreeView.
    static ref SLIGHTLY_DARKER_GREY: &'static str = "#101010";        // A Bit Darker Grey.
    static ref KINDA_WHITY_GREY: &'static str = "#BBBBBB";            // Light Grey. The color of the normal Text.
    static ref KINDA_MORE_WHITY_GREY: &'static str = "#CCCCCC";       // Lighter Grey. The color of the highlighted Text.
    static ref EVEN_MORE_WHITY_GREY: &'static str = "#FAFAFA";        // Even Lighter Grey.
    static ref BRIGHT_RED: &'static str = "#FF0000";                  // Bright Red, as our Lord.
    static ref DARK_RED: &'static str = "#FF0000";                    // Dark Red, as our face after facing our enemies.
    static ref LINK_BLUE: &'static str = "#2A82DA";                   // Blue, used for Zeldas.
    static ref ORANGE: &'static str = "#E67E22";                      // Orange, used for borders.
    static ref MEDIUM_GREY: &'static str = "#555555";

    static ref YELLOW_BRIGHT: &'static str = "#FFFFDD";
    static ref YELLOW_DARK: &'static str = "#525200";

    static ref GREEN_BRIGHT: &'static str = "#D0FDCC";
    static ref GREEN_DARK: &'static str = "#708F6E";

    static ref RED_BRIGHT: &'static str = "#FFCCCC";
    static ref RED_DARK: &'static str = "#8F6E6E";


    /// Variable to keep the locale fallback data (english locales) used by the UI loaded and available.
    static ref LOCALE_FALLBACK: Locale = {
        match Locale::initialize_fallback() {
            Ok(locale) => locale,
            Err(_) => Locale::initialize_empty(),
        }
    };

    /// Variable to keep the locale data used by the UI loaded and available. If we fail to load the selected locale data, copy the english one instead.
    static ref LOCALE: Locale = {
        match SETTINGS.read().unwrap().settings_string.get("language") {
            Some(language) => Locale::initialize(language).unwrap_or_else(|_| LOCALE_FALLBACK.clone()),
            None => LOCALE_FALLBACK.clone(),
        }
    };

    /// Global variable to hold the sender/receivers used to comunicate between threads.
    static ref CENTRAL_COMMAND: CentralCommand = CentralCommand::default();

    /// Global variable to hold certain info about the current state of the UI.
    static ref UI_STATE: UIState = UIState::default();

    /// Pointer to the status bar of the Main Window, for logging purpouses.
    static ref STATUS_BAR: AtomicPtr<QStatusBar> = unsafe { atomic_from_cpp_box(QStatusBar::new_0a()) };

    /// Monospace font, just in case we need it.
    static ref FONT_MONOSPACE: AtomicPtr<QFont> = unsafe { atomic_from_cpp_box(QFontDatabase::system_font(SystemFont::FixedFont)) };
}

/// This constant gets RPFM's version from the `Cargo.toml` file, so we don't have to change it
/// in two different places in every update.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main function.
fn main() {

    // Log the crashes so the user can send them himself.
    if !cfg!(debug_assertions) && CrashReport::init().is_err() {
        let _ = CombinedLogger::init(
            vec![
                TermLogger::new(LevelFilter::Info, simplelog::Config::default(), TerminalMode::Mixed).ok_or_else(|| Error::from(ErrorKind::InitializingLoggerError)).unwrap(),
                WriteLogger::new(LevelFilter::Info, simplelog::Config::default(), File::create(get_config_path().unwrap().join("rpfm_ui.log")).unwrap()),
            ]
        );
        info!("Starting...");
        println!("Failed to initialize logging code.");
    }

    // If the config folder doesn't exist, and we failed to initialize it, force a crash.
    // If this fails, half the program will be broken in one way or another, so better safe than sorry.
    if let Err(error) = init_config_path() { panic!(error); }

    //---------------------------------------------------------------------------------------//
    // Preparing the Program...
    //---------------------------------------------------------------------------------------//

    // Create the background and network threads, where all the magic will happen.
    thread::spawn(move || { background_thread::background_loop(); });
    thread::spawn(move || { network_thread::network_loop(); });

    // Create the application and start the loop.
    QApplication::init(|app| {
        let slot_holder = Rc::new(RefCell::new(vec![]));
        let (_ui, _slots) = unsafe { UI::new(app, &slot_holder) };

        // And launch it.
        unsafe { QApplication::exec() }
    })
}
