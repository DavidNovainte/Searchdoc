// Fails fast when the Tauri dev port is already taken - usually a leftover
// SearchDoc instance hiding in the tray (closing the window hides, not exits).
// npm runs this automatically via the "predev" hook.
import net from "node:net";

const PORT = Number(process.env.DEV_PORT ?? 1420);

const server = net.createServer();
server.once("error", (err) => {
  if (err.code !== "EADDRINUSE") {
    console.error("端口检查失败：" + err.message);
    process.exit(1);
  }
  console.error(
    "\n✗ 端口 " + PORT + " 已被占用 —— vite 配置了 strictPort，继续跑必然失败。\n\n" +
      "最常见原因：上一次 SearchDoc 没有真正退出（关闭窗口只是隐藏到托盘），\n" +
      "残留的 vite / searchdoc 进程还占着端口。处理：\n\n" +
      "  1) 找到占用进程的 PID：\n" +
      "     Get-NetTCPConnection -LocalPort " + PORT + " -State Listen | Select-Object OwningProcess\n\n" +
      "  2) 结束它（把 <PID> 换成上面的值）：\n" +
      "     Stop-Process -Id <PID> -Force\n\n" +
      "  3) 重新运行：npm run desktop\n",
  );
  process.exit(1);
});
server.once("listening", () => {
  server.close(() => {
    console.log("✓ 端口 " + PORT + " 可用，启动开发环境…");
    process.exit(0);
  });
});
server.listen(PORT, "127.0.0.1");
