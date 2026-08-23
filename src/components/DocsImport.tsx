import "../styles/DocsImport.css";
import type { Dispatch, SetStateAction } from "react";

export interface DocsImportProps {
  linkText: string;
  setLinkText: Dispatch<SetStateAction<string>>;
  googleBusy: boolean;
  setShowDocsImport: Dispatch<SetStateAction<boolean>>;
  handleImportLinks: () => void;
}

export function DocsImport(props: DocsImportProps) {
  const { linkText, setLinkText, googleBusy, setShowDocsImport, handleImportLinks } = props;
  return (
<div className="section-block inset docs-import">
  <div className="section-title row">
    <div>
      <h2>添加 Docs 链接</h2>
      <p>多行粘贴 · 导入后加入观察列表</p>
    </div>
    <button
      type="button"
      className="btn ghost sm"
      onClick={() => {
        setShowDocsImport(false);
        setLinkText("");
      }}
    >
      取消
    </button>
  </div>
  <textarea
    className="link-input"
    value={linkText}
    onChange={(e) => setLinkText(e.target.value)}
    placeholder={
      "https://docs.google.com/document/d/xxxx/edit\n可多行粘贴"
    }
    rows={3}
    disabled={googleBusy}
    autoFocus
  />
  <div className="form-actions tight">
    <button
      type="button"
      className="btn primary sm"
      disabled={googleBusy || !linkText.trim()}
      onClick={() => void handleImportLinks()}
    >
      {googleBusy ? "处理中…" : "导入并同步"}
    </button>
  </div>
</div>
  );
}
