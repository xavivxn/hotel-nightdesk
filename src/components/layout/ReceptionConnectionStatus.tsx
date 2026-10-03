import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AlertTriangle, ArrowRight } from "lucide-react";
import { api } from "@/lib/api";
import type { DeviceMode, LanHost, LanNearbyStation, LanStatus, PendingOperation } from "@/lib/types";

export type LanAdminInfo = {
  identity: LanHost | null;
  pairing_open: boolean;
  pending: { station_id: string; name: string; verification_code: string }[];
  stations: { id: string; name: string; active: boolean }[];
};

export type ReceptionLan = {
  status: LanStatus | null;
  info: LanAdminInfo | null;
  nearby: LanNearbyStation[];
  pending: PendingOperation[];
  openRequest: number;
  showPanel: () => void;
  refresh: () => Promise<void>;
  scan: () => Promise<void>;
  retry: (operationId: string) => Promise<void>;
  review: (operationId: string, note: string) => Promise<void>;
};

export function useReceptionLan(mode: DeviceMode, admin: boolean): ReceptionLan {
  const [status, setStatus] = useState<LanStatus | null>(null);
  const [info, setInfo] = useState<LanAdminInfo | null>(null);
  const [nearby, setNearby] = useState<LanNearbyStation[]>([]);
  const [pending, setPending] = useState<PendingOperation[]>([]);
  const [openRequest, setOpenRequest] = useState(0);
  const revision = useRef("");
  const polling = useRef(false);
  const scanning = useRef(false);

  const refresh = useCallback(async () => {
    if (mode === "remote" || polling.current) return;
    polling.current = true;
    try {
      const reads: Promise<unknown>[] = [
        api.receptionRevision().then(r => {
          const key = `${r.epoch}:${r.revision}`;
          if (key !== revision.current) {
            revision.current = key;
            window.dispatchEvent(new Event("reception:changed"));
          }
        }),
        api.lanStatus().then(setStatus),
      ];
      if (mode === "reception_client") {
        reads.push(api.pendingOperations().then(setPending));
      }
      if (mode === "reception" && admin) {
        reads.push(api.lanControl<LanAdminInfo>("admin_info").then(setInfo));
      }
      // A failed read never hides the last known unresolved request or stops other reads.
      await Promise.allSettled(reads);
    } finally {
      polling.current = false;
    }
  }, [mode, admin]);

  const scan = useCallback(async () => {
    if (mode !== "reception" || !admin || scanning.current) return;
    scanning.current = true;
    try { setNearby(await api.discoverAdditionalStations()); }
    finally { scanning.current = false; }
  }, [mode, admin]);

  useEffect(() => {
    if (mode === "remote") return;
    void refresh();
    const id = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(id);
  }, [mode, refresh]);

  useEffect(() => {
    if (mode !== "reception" || !admin) return;
    void scan().catch(() => undefined);
    const id = window.setInterval(() => void scan().catch(() => undefined), 10000);
    return () => window.clearInterval(id);
  }, [mode, admin, scan]);

  const retry = useCallback(async (operationId: string) => {
    if (mode !== "reception_client") return;
    setPending(await api.retryOperation(operationId));
  }, [mode]);

  const review = useCallback(async (operationId: string, note: string) => {
    if (mode !== "reception_client" || !admin) return;
    await api.lanControl("review_pending", { operation_id: operationId, note });
    setPending(await api.pendingOperations());
  }, [mode, admin]);

  return { status, info, nearby, pending, openRequest, showPanel: () => setOpenRequest(v => v + 1), refresh, scan, retry, review };
}

export function ReceptionLanAlert({ mode, lan }: { mode: DeviceMode; lan: ReceptionLan }) {
  const navigate = useNavigate();
  const [showLoss, setShowLoss] = useState(false);
  const pending = lan.pending.length;
  const lost = mode === "reception_client" && !!lan.status?.paired && !lan.status.connection.connected;
  const serverDown = mode === "reception" && !!lan.status?.enabled && !lan.status.running;
  useEffect(() => {
    if (!lost && !serverDown) { setShowLoss(false); return; }
    setShowLoss(true);
    const timer = window.setTimeout(() => setShowLoss(false), 7000);
    return () => window.clearTimeout(timer);
  }, [lost, serverDown]);
  if (mode === "remote" || (!pending && !showLoss)) return null;
  const message = pending
    ? `${pending} operación${pending === 1 ? "" : "es"} sin confirmar`
    : lost ? "Sin conexión con la principal" : "Servicio LAN no disponible";
  return <div className={`lan-alert ${pending ? "is-pending" : ""}`} role="alert">
    <AlertTriangle size={17} aria-hidden="true" />
    <span>{message}</span>
    <button type="button" onClick={() => { navigate("/"); lan.showPanel(); }} aria-label={`Ver detalle: ${message}`}>
      Ver detalle <ArrowRight size={15} aria-hidden="true" />
    </button>
  </div>;
}
