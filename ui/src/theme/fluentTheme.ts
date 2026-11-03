import { webDarkTheme, webLightTheme, type Theme } from "@fluentui/react-components";
import { themeLight, type DockviewTheme } from "dockview-react";

/**
 * Fluent v9 themes built from the design tokens in tokens.css. Every colour is a `var(--token)`, so Fluent controls, dockview
 * and our own CSS read the same values and follow `data-theme` and the user's colour choices live. Only the base theme differs
 * between light and dark, for the tokens we do not map.
 */
const v = (name: string) => `var(--${name})`;
const mixText = (pct: number, base = "transparent") => `color-mix(in srgb, var(--text) ${pct}%, ${base})`;
const mixLink = (pct: number) => `color-mix(in srgb, var(--link) ${100 - pct}%, var(--text))`;

const mapped: Partial<Theme> = {
  fontFamilyBase: v("font-ui"),
  fontFamilyMonospace: v("font-mono"),
  borderRadiusSmall: v("r-control"),
  borderRadiusMedium: v("r-control"),
  borderRadiusLarge: "3px",
  borderRadiusXLarge: "4px",

  colorNeutralBackground1: v("surface-2"),
  colorNeutralBackground1Hover: `color-mix(in srgb, var(--surface-2) 92%, var(--text))`,
  colorNeutralBackground1Pressed: `color-mix(in srgb, var(--surface-2) 85%, var(--text))`,
  colorNeutralBackground1Selected: `color-mix(in srgb, var(--surface-2) 88%, var(--text))`,
  colorNeutralBackground2: v("surface-1"),
  colorNeutralBackground2Hover: `color-mix(in srgb, var(--surface-1) 92%, var(--text))`,
  colorNeutralBackground2Pressed: `color-mix(in srgb, var(--surface-1) 85%, var(--text))`,
  colorNeutralBackground2Selected: `color-mix(in srgb, var(--surface-1) 88%, var(--text))`,
  colorNeutralBackground3: v("surface-0"),
  colorNeutralBackground3Hover: `color-mix(in srgb, var(--surface-0) 92%, var(--text))`,
  colorNeutralBackground3Pressed: `color-mix(in srgb, var(--surface-0) 85%, var(--text))`,
  colorNeutralBackground3Selected: `color-mix(in srgb, var(--surface-0) 88%, var(--text))`,
  colorNeutralBackground4: v("surface-0"),
  colorNeutralBackground5: v("surface-0"),
  colorNeutralBackground6: v("surface-0"),
  colorNeutralBackgroundDisabled: v("surface-0"),
  colorNeutralBackgroundInverted: v("text"),
  colorSubtleBackground: "transparent",
  colorSubtleBackgroundHover: v("hover"),
  colorSubtleBackgroundPressed: mixText(14),
  colorSubtleBackgroundSelected: mixText(12),
  colorTransparentBackgroundHover: v("hover"),
  colorTransparentBackgroundPressed: mixText(14),
  colorTransparentBackgroundSelected: mixText(12),
  colorBackgroundOverlay: v("scrim"),

  colorNeutralForeground1: v("text"),
  colorNeutralForeground1Hover: v("text"),
  colorNeutralForeground1Pressed: v("text"),
  colorNeutralForeground1Selected: v("text"),
  colorNeutralForeground2: v("text"),
  colorNeutralForeground2Hover: v("text"),
  colorNeutralForeground2Pressed: v("text"),
  colorNeutralForeground2Selected: v("text"),
  colorNeutralForeground2BrandHover: v("link"),
  colorNeutralForeground2BrandPressed: v("link"),
  colorNeutralForeground2BrandSelected: v("link"),
  colorNeutralForeground3: v("text-2"),
  colorNeutralForeground3Hover: v("text"),
  colorNeutralForeground3Pressed: v("text"),
  colorNeutralForeground3Selected: v("text"),
  colorNeutralForeground4: v("text-2"),
  colorNeutralForegroundDisabled: `color-mix(in srgb, var(--text-2) 60%, var(--surface-1))`,
  colorNeutralForegroundInverted: v("surface-2"),
  colorNeutralForegroundOnBrand: v("surface-2"),
  colorNeutralForegroundStaticInverted: v("on-dark"),

  colorNeutralStroke1: v("rule"),
  colorNeutralStroke1Hover: v("rule"),
  colorNeutralStroke1Pressed: v("rule"),
  colorNeutralStroke1Selected: v("rule"),
  colorNeutralStroke2: v("rule-soft"),
  colorNeutralStroke3: v("rule-soft"),
  colorNeutralStrokeAccessible: v("rule"),
  colorNeutralStrokeAccessibleHover: v("rule"),
  colorNeutralStrokeAccessiblePressed: v("rule"),
  colorNeutralStrokeAccessibleSelected: v("link"),
  colorNeutralStrokeDisabled: v("rule-soft"),
  colorNeutralStrokeSubtle: v("rule-soft"),
  colorStrokeFocus1: v("surface-2"),
  colorStrokeFocus2: v("focus"),

  colorBrandBackground: v("link"),
  colorBrandBackgroundHover: mixLink(15),
  colorBrandBackgroundPressed: mixLink(30),
  colorBrandBackgroundSelected: mixLink(15),
  colorBrandBackground2: v("info-bg"),
  colorBrandBackground2Hover: v("info-bg"),
  colorCompoundBrandBackground: v("link"),
  colorCompoundBrandBackgroundHover: mixLink(15),
  colorCompoundBrandBackgroundPressed: mixLink(30),
  colorCompoundBrandStroke: v("link"),
  colorCompoundBrandStrokeHover: mixLink(15),
  colorCompoundBrandStrokePressed: mixLink(30),
  colorBrandForeground1: v("link"),
  colorBrandForeground2: v("link"),
  colorBrandForegroundLink: v("link"),
  colorBrandForegroundLinkHover: mixLink(15),
  colorBrandForegroundLinkPressed: mixLink(30),
  colorBrandForegroundLinkSelected: v("link"),
  colorCompoundBrandForeground1: v("link"),
  colorCompoundBrandForeground1Hover: mixLink(15),
  colorCompoundBrandForeground1Pressed: mixLink(30),
  colorBrandStroke1: v("link"),
  colorBrandStroke2: v("rule-soft"),

  colorStatusDangerForeground1: v("error"),
  colorStatusDangerForeground3: v("error"),
  colorStatusDangerBackground1: v("error-bg"),
  colorStatusDangerBorder1: v("error"),
  colorStatusWarningForeground1: v("warn"),
  colorStatusWarningForeground3: v("warn"),
  colorStatusWarningBackground1: v("warn-bg"),
  colorStatusWarningBorder1: v("warn"),
  colorStatusSuccessForeground1: v("ok"),
  colorStatusSuccessForeground3: v("ok"),
  colorStatusSuccessBackground1: v("ok-bg"),
  colorStatusSuccessBorder1: v("ok"),
  colorPaletteRedForeground1: v("error"),
  colorPaletteRedForeground3: v("error"),
  colorPaletteRedBorder2: v("error"),
  colorPaletteDarkOrangeForeground1: v("warn"),
  colorPaletteYellowForeground2: v("warn"),
  colorPaletteGreenForeground1: v("ok"),
  colorPaletteGreenForeground3: v("ok"),

  // Docked things have no elevation, only rules; floating things share one shadow.
  shadow2: "none",
  shadow4: "none",
  shadow8: v("e-float"),
  shadow16: v("e-float"),
  shadow28: v("e-float"),
  shadow64: v("e-float"),

  durationFaster: v("dur-menu"),
  durationFast: v("dur-menu"),
  durationNormal: v("dur-panel"),
};

export const fluentLight: Theme = { ...webLightTheme, ...mapped };
export const fluentDark: Theme = { ...webDarkTheme, ...mapped };

/** Dockview's own light class is used in both modes; its variables are set from tokens in dock.css. */
export const dockTheme: DockviewTheme = themeLight;

export type ThemeMode = "light" | "dark";

/** Puts the mode on <html>, not only on .app, so portalled menus and Fluent popups get the tokens too. */
export function applyThemeMode(mode: ThemeMode): void {
  document.documentElement.dataset.theme = mode;
}
