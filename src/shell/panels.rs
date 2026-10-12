//! Panel registry: every sidebar panel, dockable in either dock.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PanelKind {
    Project,
    Outline,
    Styling,
    Inspector,
    History,
    Comments,
}

impl PanelKind {
    pub fn all() -> [PanelKind; 6] {
        [
            PanelKind::Project,
            PanelKind::Outline,
            PanelKind::Styling,
            PanelKind::Inspector,
            PanelKind::History,
            PanelKind::Comments,
        ]
    }

    /// Stable storage key for persistence.
    pub fn key(self) -> &'static str {
        match self {
            PanelKind::Project => "project",
            PanelKind::Outline => "outline",
            PanelKind::Styling => "styling",
            PanelKind::Inspector => "inspector",
            PanelKind::History => "history",
            PanelKind::Comments => "comments",
        }
    }

    pub fn from_key(s: &str) -> Option<PanelKind> {
        match s {
            "project" => Some(PanelKind::Project),
            "outline" => Some(PanelKind::Outline),
            "styling" => Some(PanelKind::Styling),
            "inspector" => Some(PanelKind::Inspector),
            "history" => Some(PanelKind::History),
            "comments" => Some(PanelKind::Comments),
            _ => None,
        }
    }

    /// Short toggle label used for element ids.
    pub fn short_label(self) -> &'static str {
        match self {
            PanelKind::Project => "Project",
            PanelKind::Outline => "Outline",
            PanelKind::Styling => "Styles",
            PanelKind::Inspector => "Inspect",
            PanelKind::History => "History",
            PanelKind::Comments => "Notes",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockSide {
    Left,
    Right,
}

impl DockSide {
    pub fn other(self) -> DockSide {
        match self {
            DockSide::Left => DockSide::Right,
            DockSide::Right => DockSide::Left,
        }
    }
}

impl PanelKind {
    pub fn icon(self) -> &'static [u8] {
        match self {
            PanelKind::Project => ICON_PROJECT,
            PanelKind::Outline => ICON_OUTLINE,
            PanelKind::Styling => ICON_STYLING,
            PanelKind::Inspector => ICON_INSPECTOR,
            PanelKind::History => ICON_HISTORY,
            PanelKind::Comments => ICON_COMMENTS,
        }
    }
}

pub const ICON_PROJECT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M176 150h32a14 14 0 0 0 14-14v-32a14 14 0 0 0-14-14h-32a14 14 0 0 0-14 14v10H86V78h10a14 14 0 0 0 14-14V32a14 14 0 0 0-14-14H64a14 14 0 0 0-14 14v32a14 14 0 0 0 14 14h10v114a22 22 0 0 0 22 22h66v10a14 14 0 0 0 14 14h32a14 14 0 0 0 14-14v-32a14 14 0 0 0-14-14h-32a14 14 0 0 0-14 14v10H96a10 10 0 0 1-10-10v-66h76v10a14 14 0 0 0 14 14M62 64V32a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2H64a2 2 0 0 1-2-2m112 128a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2h-32a2 2 0 0 1-2-2Zm0-88a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2h-32a2 2 0 0 1-2-2Z" />
</svg>"#;

pub const ICON_STYLING: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M229.23 26.77a30.05 30.05 0 0 0-42.43 0l-.32.34l-54.81 64.05l-12.11-12.1a22 22 0 0 0-31.11 0l-76.69 76.69a6 6 0 0 0 0 8.49l80 80a6 6 0 0 0 8.49 0l76.69-76.69a22 22 0 0 0 0-31.11l-12.1-12.11l64-54.81l.34-.32a30.05 30.05 0 0 0 .05-42.43M96 231.51L76.49 212l23.76-23.76a6 6 0 0 0-8.49-8.49L68 203.51L52.49 188l23.76-23.76a6 6 0 0 0-8.49-8.49L44 179.51L24.49 160L72 112.48L143.52 184ZM220.89 60.56l-68.78 58.87a6 6 0 0 0-2.1 4.33a6 6 0 0 0 1.76 4.47l16.68 16.69a10 10 0 0 1 0 14.15L152 175.51L80.49 104l16.44-16.45a10 10 0 0 1 14.15 0l16.69 16.68a6 6 0 0 0 8.8-.34l58.87-68.78a18 18 0 0 1 25.45 25.45" />
</svg>"#;

pub const ICON_INSPECTOR: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M230 208a6 6 0 0 1-6 6H32a6 6 0 0 1-6-6V48a6 6 0 0 1 12 0v98.78l54.05-47.3a6 6 0 0 1 7.55-.28l60.11 45.08l60.34-52.8a6 6 0 0 1 7.9 9l-64 56a6 6 0 0 1-7.55.28l-60.11-45.04l-58.29 51V202h186a6 6 0 0 1 6 6" />
</svg>"#;

pub const ICON_HISTORY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M98 136a6 6 0 0 1 6-6h64a6 6 0 0 1 0 12h-64a6 6 0 0 1-6-6m6-26h64a6 6 0 0 0 0-12h-64a6 6 0 0 0 0 12m126 82a30 30 0 0 1-30 30H88a30 30 0 0 1-30-30V64a18 18 0 0 0-36 0c0 6.76 5.58 11.19 5.64 11.23a6 6 0 1 1-7.24 9.57C20 84.48 10 76.85 10 64a30 30 0 0 1 30-30h136a30 30 0 0 1 30 30v106h10a6 6 0 0 1 3.6 1.2c.4.32 10.4 7.95 10.4 20.8m-124 0c0-6.76-5.59-11.19-5.64-11.23A6 6 0 0 1 104 170h90V64a18 18 0 0 0-18-18H64a29.82 29.82 0 0 1 6 18v128a18 18 0 0 0 36 0m112 0a14.94 14.94 0 0 0-4.34-10h-97.78a24.8 24.8 0 0 1 2.12 10a29.87 29.87 0 0 1-6 18h88a18 18 0 0 0 18-18" />
</svg>"#;

pub const ICON_COMMENTS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M138 128a10 10 0 1 1-10-10a10 10 0 0 1 10 10m-54-10a10 10 0 1 0 10 10a10 10 0 0 0-10-10m88 0a10 10 0 1 0 10 10a10 10 0 0 0-10-10m58 10a102 102 0 0 1-150.69 89.65l-34.87 11.62a14 14 0 0 1-17.71-17.71l11.62-34.87A102 102 0 1 1 230 128m-12 0a90 90 0 1 0-167.92 45.06a6 6 0 0 1 .5 4.91l-12.46 37.38a2 2 0 0 0 2.53 2.53L78 205.42a6.2 6.2 0 0 1 1.9-.31a6.1 6.1 0 0 1 3 .81A90 90 0 0 0 218 128" />
</svg>"#;

pub const ICON_OUTLINE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M90 64a6 6 0 0 1 6-6h120a6 6 0 0 1 0 12H96a6 6 0 0 1-6-6m126 58H96a6 6 0 0 0 0 12h120a6 6 0 0 0 0-12m0 64H96a6 6 0 0 0 0 12h120a6 6 0 0 0 0-12M56 58H40a6 6 0 0 0 0 12h16a6 6 0 0 0 0-12m0 64H40a6 6 0 0 0 0 12h16a6 6 0 0 0 0-12m0 64H40a6 6 0 0 0 0 12h16a6 6 0 0 0 0-12" />
</svg>"#;

pub const ICON_SYNTAX: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<g fill="currentColor">
		<path d="M240 128c-64 0 0 88-64 88H80c-64 0 0-88-64-88c64 0 0-88 64-88h96c64 0 0 88 64 88" opacity=".2" />
		<path d="M43.18 128a29.8 29.8 0 0 1 8 10.26c4.8 9.9 4.8 22 4.8 33.74c0 24.31 1 36 24 36a8 8 0 0 1 0 16c-17.48 0-29.32-6.14-35.2-18.26c-4.8-9.9-4.8-22-4.8-33.74c0-24.31-1-36-24-36a8 8 0 0 1 0-16c23 0 24-11.69 24-36c0-11.72 0-23.84 4.8-33.74C50.68 38.14 62.52 32 80 32a8 8 0 0 1 0 16c-23 0-24 11.69-24 36c0 11.72 0 23.84-4.8 33.74A29.8 29.8 0 0 1 43.18 128M240 120c-23 0-24-11.69-24-36c0-11.72 0-23.84-4.8-33.74C205.32 38.14 193.48 32 176 32a8 8 0 0 0 0 16c23 0 24 11.69 24 36c0 11.72 0 23.84 4.8 33.74a29.8 29.8 0 0 0 8 10.26a29.8 29.8 0 0 0-8 10.26c-4.8 9.9-4.8 22-4.8 33.74c0 24.31-1 36-24 36a8 8 0 0 0 0 16c17.48 0 29.32-6.14 35.2-18.26c4.8-9.9 4.8-22 4.8-33.74c0-24.31 1-36 24-36a8 8 0 0 0 0-16" />
	</g>
</svg>"#;

pub const ICON_PROSE_LAYER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M246 92.68a13.94 13.94 0 0 0-4.1-9.9L173.21 14.1a14 14 0 0 0-19.8 0l-28.73 28.73l-58.46 21.93a14 14 0 0 0-8.9 10.8L34.08 215a6 6 0 0 0 5.92 7a6.6 6.6 0 0 0 1-.08l139.44-23.24a14 14 0 0 0 10.81-8.9l21.92-58.46l28.74-28.74a13.92 13.92 0 0 0 4.09-9.9m-66 92.89a2 2 0 0 1-1.54 1.27L57.49 207l52.87-52.88a26 26 0 1 0-8.48-8.48L49 198.53l20.17-121A2 2 0 0 1 70.43 76l56.06-21L201 129.51ZM110 132a14 14 0 1 1 14 14a14 14 0 0 1-14-14m123.41-37.9L208 119.51L136.48 48l25.42-25.42a2 2 0 0 1 2.83 0l68.68 68.69a2 2 0 0 1 0 2.83" />
</svg>"#;
