import "../styles/WatchList.css";
import type { GoogleAuthStatus, GoogleWatchItem } from "../types";

export interface WatchListProps {
  watchlist: GoogleWatchItem[];
  googleBusy: boolean;
  googleAuth: GoogleAuthStatus | null;
  handleSyncWatchlist: () => void;
  handleRemoveWatch: (id: string) => void;
}

export function WatchList(props: WatchListProps) {
  const { watchlist, googleBusy, googleAuth, handleSyncWatchlist, handleRemoveWatch } = props;
  return (
<div className="section-block inset">
  <div className="section-title row">
    <div>
      <h2>观察列表</h2>
      <p>{watchlist.length} 个指定 Docs</p>
    </div>
    <button
      type="button"
      className="btn ghost sm"
      disabled={googleBusy || !googleAuth?.connected}
      onClick={() => void handleSyncWatchlist()}
    >
      同步列表
    </button>
  </div>
  <div className="watch-list">
    {watchlist.map((item) => (
      <div key={item.id} className="watch-item">
        <div className="watch-main">
          <strong>{item.title || item.id}</strong>
          <span className="mono small">{item.url}</span>
        </div>
        <button
          type="button"
          className="btn danger sm"
          disabled={googleBusy}
          onClick={() => void handleRemoveWatch(item.id)}
        >
          移除
        </button>
      </div>
    ))}
  </div>
</div>
  );
}
