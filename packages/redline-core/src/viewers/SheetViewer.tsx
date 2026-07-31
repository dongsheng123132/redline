import { useEffect, useState } from "react";
import type { ViewerProps } from "./types";
import type { RedlineUnit } from "../document-model";
import { numberedUnitId, numberedUnitIndex } from "./util";

const MAX_ROWS = 500; // 大表格截断，不做虚拟滚动（不用太深入）

/** Excel 网格预览 —— 自己拼 <table>（React 转义单元格内容，不用 sheet_to_html 那种可能透传原值到 innerHTML 的路径）。 */
export default function SheetViewer({ bytes, activeUnitId, onUnitsResolved, renderOverlay }: ViewerProps) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [workbook, setWorkbook] = useState<any>(null);
  const [err, setErr] = useState<string | null>(null);
  const [rows, setRows] = useState<unknown[][]>([]);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const XLSX = await import("xlsx");
        const wb = XLSX.read(bytes, { type: "array" });
        if (cancelled) return;
        const units: RedlineUnit[] = wb.SheetNames.map((name: string, i: number) => ({
          id: numberedUnitId("sheet", i),
          kind: "sheet",
          index: i,
          label: name,
        }));
        onUnitsResolved(units);
        setWorkbook(wb);
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [bytes, onUnitsResolved]);

  useEffect(() => {
    if (!workbook || !activeUnitId) return;
    let cancelled = false;
    (async () => {
      const XLSX = await import("xlsx");
      const sheetIndex = numberedUnitIndex(activeUnitId, "sheet");
      if (sheetIndex < 0) return;
      const sheetName = workbook.SheetNames[sheetIndex];
      const sheet = sheetName ? workbook.Sheets[sheetName] : undefined;
      const data = sheet
        ? (XLSX.utils.sheet_to_json(sheet, { header: 1, defval: "" }) as unknown[][])
        : [];
      if (cancelled) return;
      const clipped = data.slice(0, MAX_ROWS);
      setRows(clipped);
      const colCount = clipped.reduce((m, r) => Math.max(m, r.length), 1);
      setSize({ width: colCount * 96, height: clipped.length * 28 });
    })();
    return () => {
      cancelled = true;
    };
  }, [workbook, activeUnitId]);

  if (err) {
    return <div className="p-4 text-red-400 text-[12px]">Excel 解析失败：{err}</div>;
  }

  return (
    <div className="relative flex-1 overflow-auto min-h-0 bg-white text-black text-[12px]">
      <table className="border-collapse">
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri}>
              {row.map((cell, ci) => (
                <td key={ci} className="border border-gray-300 px-2 py-1 whitespace-nowrap">
                  {String(cell)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {size && <div className="absolute inset-0">{renderOverlay(activeUnitId, size)}</div>}
    </div>
  );
}
