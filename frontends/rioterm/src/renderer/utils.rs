use crate::constants;
use crate::layout::ContextDimension;
use rio_backend::config::navigation::Navigation;
use rio_backend::config::Config;
use rio_window::window::Theme;

#[inline]
pub fn padding_top_from_config(
    navigation: &Navigation,
    padding_y_top: f32,
    #[allow(unused)] num_tabs: usize,
    #[allow(unused)] macos_use_unified_titlebar: bool,
) -> f32 {
    // When navigation is enabled (Tab mode), start content below island
    if navigation.is_enabled() {
        // On Linux/Windows, if hide_if_single is true and there's only one tab,
        // the island is hidden so render from 0 + configured margin
        #[cfg(not(target_os = "macos"))]
        if navigation.hide_if_single && num_tabs <= 1 {
            return constants::PADDING_Y + padding_y_top;
        }

        use crate::renderer::island::ISLAND_HEIGHT;
        return ISLAND_HEIGHT + padding_y_top;
    }

    let default_padding = constants::PADDING_Y + padding_y_top;

    #[cfg(target_os = "macos")]
    {
        use rio_backend::config::navigation::NavigationMode;
        if navigation.mode == NavigationMode::NativeTab {
            let additional = if macos_use_unified_titlebar {
                constants::ADDITIONAL_PADDING_Y_ON_UNIFIED_TITLEBAR
            } else {
                0.0
            };
            return additional + padding_y_top;
        }
    }

    default_padding
}

#[inline]
pub fn terminal_dimensions(layout: &ContextDimension) -> teletypewriter::WinsizeBuilder {
    let scale = layout.dimension.scale;
    let width = (layout.width - layout.margin.left * scale - layout.margin.right * scale)
        .max(0.0);
    let height =
        (layout.height - layout.margin.top * scale - layout.margin.bottom * scale)
            .max(0.0);

    teletypewriter::WinsizeBuilder {
        width: width as u16,
        height: height as u16,
        cols: layout.columns as u16,
        rows: layout.lines as u16,
    }
}

#[inline]
pub fn update_colors_based_on_theme(config: &mut Config, theme_opt: Option<Theme>) {
    if let Some(theme) = theme_opt {
        if let Some(adaptive_colors) = &config.adaptive_colors {
            match theme {
                Theme::Light => {
                    if let Some(light_colors) = adaptive_colors.light {
                        config.colors = light_colors;
                    }
                }
                Theme::Dark => {
                    if let Some(darkcolors) = adaptive_colors.dark {
                        config.colors = darkcolors;
                    }
                }
            }
        }
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use rio_backend::config::navigation::{Navigation, NavigationMode};

    fn tab_navigation(hide_if_single: bool) -> Navigation {
        Navigation {
            mode: NavigationMode::Tab,
            hide_if_single,
            ..Navigation::default()
        }
    }

    #[test]
    fn padding_top_hides_tab_island_for_one_tab_when_configured() {
        let margin_top = 5.0;

        assert_eq!(
            padding_top_from_config(&tab_navigation(true), margin_top, 1, false),
            crate::constants::PADDING_Y + margin_top
        );
    }

    #[test]
    fn padding_top_keeps_tab_island_for_two_tabs_when_single_tab_hidden() {
        let margin_top = 5.0;

        assert_eq!(
            padding_top_from_config(&tab_navigation(true), margin_top, 2, false),
            crate::renderer::island::ISLAND_HEIGHT + margin_top
        );
    }

    #[test]
    fn padding_top_keeps_tab_island_for_one_tab_when_not_hiding_single_tab() {
        let margin_top = 5.0;

        assert_eq!(
            padding_top_from_config(&tab_navigation(false), margin_top, 1, false),
            crate::renderer::island::ISLAND_HEIGHT + margin_top
        );
    }
}
