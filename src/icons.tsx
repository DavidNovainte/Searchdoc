import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function baseProps({ size = 18, className, ...rest }: IconProps) {
  return {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.75,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    className: className ? `ui-icon ${className}` : "ui-icon",
    "aria-hidden": true as const,
    ...rest,
  };
}

export function IconSearch(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <circle cx="11" cy="11" r="6.5" />
      <path d="M16.5 16.5 21 21" />
    </svg>
  );
}

export function IconSources(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M4 7h16" />
      <path d="M4 12h16" />
      <path d="M4 17h10" />
      <circle cx="18.5" cy="17" r="1.5" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function IconChevronLeft(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M14.5 6 9 12l5.5 6" />
    </svg>
  );
}

export function IconChevronRight(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M9.5 6 15 12l-5.5 6" />
    </svg>
  );
}

export function IconFolder(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M3.5 8.5A1.5 1.5 0 0 1 5 7h4.2l1.6 1.6H19A1.5 1.5 0 0 1 20.5 10v7.5A1.5 1.5 0 0 1 19 19H5a1.5 1.5 0 0 1-1.5-1.5v-9Z" />
    </svg>
  );
}

export function IconCloud(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M7.5 17.5h9.2a3.8 3.8 0 0 0 .4-7.58 5.2 5.2 0 0 0-10-1.3A3.6 3.6 0 0 0 7.5 17.5Z" />
    </svg>
  );
}

export function IconLink(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M9.5 14.5 14.5 9.5" />
      <path d="M11 8.2 12.4 6.8a3.2 3.2 0 1 1 4.5 4.5L15.5 12.7" />
      <path d="M13 15.8 11.6 17.2a3.2 3.2 0 1 1-4.5-4.5L8.5 11.3" />
    </svg>
  );
}

export function IconClose(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M7 7 17 17" />
      <path d="M17 7 7 17" />
    </svg>
  );
}

export function IconSettings(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3.5v2.2M12 18.3v2.2M4.9 6.4l1.6 1.6M17.5 16l1.6 1.6M3.5 12h2.2M18.3 12h2.2M4.9 17.6l1.6-1.6M17.5 8l1.6-1.6" />
    </svg>
  );
}

export function IconSync(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M4.5 12a7.5 7.5 0 0 1 12.7-5.4L19 5v5h-5" />
      <path d="M19.5 12a7.5 7.5 0 0 1-12.7 5.4L5 19v-5h5" />
    </svg>
  );
}

export function IconUser(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <circle cx="12" cy="9" r="3.2" />
      <path d="M5.5 19.2c1.4-2.6 3.7-4 6.5-4s5.1 1.4 6.5 4" />
    </svg>
  );
}

export function IconExternal(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M14 5h5v5" />
      <path d="M10 14 19 5" />
      <path d="M19 13.5V18a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 18V7A1.5 1.5 0 0 1 6.5 5.5H11" />
    </svg>
  );
}

export function IconCopy(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <rect x="8.5" y="8.5" width="10" height="10" rx="1.5" />
      <path d="M6.5 15.5H6A1.5 1.5 0 0 1 4.5 14V6A1.5 1.5 0 0 1 6 4.5h8A1.5 1.5 0 0 1 15.5 6v.5" />
    </svg>
  );
}

export function IconDrive(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M4 8.5h16l-1.2 8.2A1.5 1.5 0 0 1 17.3 18H6.7a1.5 1.5 0 0 1-1.5-1.3L4 8.5Z" />
      <path d="M4 8.5 6.2 5.8A1.5 1.5 0 0 1 7.4 5.2h9.2a1.5 1.5 0 0 1 1.2.6L20 8.5" />
      <circle cx="15.5" cy="13.2" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function IconFileText(p: IconProps) {
  return (
    <svg {...baseProps(p)}>
      <path d="M7 4.5h6.5L17.5 9v10.5A1 1 0 0 1 16.5 20.5h-9A1 1 0 0 1 6.5 19.5v-14A1 1 0 0 1 7.5 4.5" />
      <path d="M13.5 4.5V9H18" />
      <path d="M9 12.5h6M9 15.5h6" />
    </svg>
  );
}
