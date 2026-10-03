import { useEffect, useRef } from "react";
import { api } from "./api";
export function useOperationalRefresh(refresh: () => void | Promise<void>) {
  const latest = useRef(refresh); latest.current = refresh;
  useEffect(() => {
    let disposed = false; let unsub = () => {}; let loading = false; let again = false;
    const run = async () => {
      if (loading) { again = true; return; }
      loading = true;
      do { again = false; try { await latest.current(); } catch { /* keep last successful data */ } } while (again && !disposed);
      loading = false;
    };
    void api.subscribeOperational(() => { void run(); }).then(fn => { if (disposed) fn(); else unsub = fn; });
    const timer = window.setInterval(() => { void run(); }, 5000);
    return () => { disposed = true; unsub(); clearInterval(timer); };
  }, []);
}
