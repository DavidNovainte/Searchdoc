import "../styles/DrivePicker.css";
import type { Dispatch, SetStateAction } from "react";
import type { LocalDriveInfo, SourceInfo } from "../types";
import { IconDrive } from "../icons";

export interface DrivePickerProps {
  localDrives: LocalDriveInfo[];
  sources: SourceInfo[];
  busy: boolean;
  setShowDrivePicker: Dispatch<SetStateAction<boolean>>;
  handleAddDrive: (drive: LocalDriveInfo) => void;
}

export function DrivePicker(props: DrivePickerProps) {
  const { localDrives, sources, busy, setShowDrivePicker, handleAddDrive } = props;
  return (
<div className="section-block inset drive-picker">
  <div className="section-title row">
    <div>
      <h2>添加整盘</h2>
      <p>
        索引盘内支持的文档类型；自动跳过 Windows / Program
        Files 等系统目录。首次全盘可能较慢。
      </p>
    </div>
    <button
      type="button"
      className="btn ghost sm"
      onClick={() => setShowDrivePicker(false)}
    >
      取消
    </button>
  </div>
  {localDrives.length === 0 ? (
    <p className="settings-hint">未检测到磁盘</p>
  ) : (
    <div className="drive-grid">
      {localDrives.map((d) => {
        const already = sources.some(
          (s) =>
            s.kind === "local" &&
            s.root_path &&
            s.root_path.replace(/[/\\]+$/, "").toLowerCase() ===
              d.path.replace(/[/\\]+$/, "").toLowerCase(),
        );
        return (
          <button
            key={d.path}
            type="button"
            className="drive-card"
            disabled={busy || already}
            onClick={() => void handleAddDrive(d)}
            title={d.path}
          >
            <span className="drive-card-ico">
              <IconDrive size={18} />
            </span>
            <strong>{d.label}</strong>
            <span>{already ? "已添加" : d.path}</span>
          </button>
        );
      })}
    </div>
  )}
</div>
  );
}
