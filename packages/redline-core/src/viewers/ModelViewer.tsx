import { useEffect, useRef, useState } from "react";
import type { ViewerProps } from "./types";
import { singleUnit } from "./util";
// 类型专用 import：不会被打包/不产生运行时体积，只给 THREE.Object3D 这类类型标注用；
// 真正加载 three.js 走下面函数体里的 await import("three")（懒加载，不进主 bundle）。
import type * as THREE from "three";

const CANVAS_W = 800;
const CANVAS_H = 600;

/**
 * 3D / 2D CAD 预览入口。stl/obj/gltf/glb 走 three.js（可交互旋转/缩放，纯预览不导出修改）；
 * dxf 走 dxf-parser + 2D canvas 手绘线条（不引入完整 CAD 内核，够画出图纸轮廓即可）。
 * STEP/IGES 等重型 CAD 交换格式 v1 不做（occt-import-js 体积大，留作 v1.1）。
 */
export default function ModelViewer(props: ViewerProps) {
  if (props.doc.format === "cad2d") return <Cad2DView {...props} />;
  return <Model3DView {...props} />;
}

function Model3DView({ doc, bytes, activeUnitId, onUnitsResolved, renderOverlay }: ViewerProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    onUnitsResolved([singleUnit("3D 模型", { width: CANVAS_W, height: CANVAS_H })]);
  }, [onUnitsResolved]);

  useEffect(() => {
    let cancelled = false;
    let cleanup: (() => void) | undefined;
    setReady(false);
    (async () => {
      try {
        const THREE = await import("three");
        const { OrbitControls } = await import("three/addons/controls/OrbitControls.js");
        const ext = doc.sourcePath.split(".").pop()?.toLowerCase();

        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x1a1a1a);
        const camera = new THREE.PerspectiveCamera(50, CANVAS_W / CANVAS_H, 0.01, 10000);
        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(CANVAS_W, CANVAS_H);
        scene.add(new THREE.AmbientLight(0xffffff, 0.6));
        const dir = new THREE.DirectionalLight(0xffffff, 0.8);
        dir.position.set(1, 1, 1);
        scene.add(dir);

        let object: THREE.Object3D;
        if (ext === "stl") {
          const { STLLoader } = await import("three/addons/loaders/STLLoader.js");
          const geometry = new STLLoader().parse(bytes);
          object = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: 0x8899aa }));
        } else if (ext === "obj") {
          const { OBJLoader } = await import("three/addons/loaders/OBJLoader.js");
          object = new OBJLoader().parse(new TextDecoder().decode(bytes));
        } else {
          const { GLTFLoader } = await import("three/addons/loaders/GLTFLoader.js");
          const gltf = await new GLTFLoader().parseAsync(bytes.slice(0), "");
          object = gltf.scene;
        }
        if (cancelled) return;
        scene.add(object);

        // 包围盒适配相机，让模型一开始就完整入画，不用手动缩放找模型
        const box = new THREE.Box3().setFromObject(object);
        const size = box.getSize(new THREE.Vector3());
        const center = box.getCenter(new THREE.Vector3());
        const maxDim = Math.max(size.x, size.y, size.z) || 1;
        camera.position.set(center.x + maxDim, center.y + maxDim, center.z + maxDim);
        camera.lookAt(center);

        const controls = new OrbitControls(camera, renderer.domElement);
        controls.target.copy(center);
        controls.update();
        const render = () => renderer.render(scene, camera);
        controls.addEventListener("change", render);
        render();

        const host = mountRef.current;
        host?.appendChild(renderer.domElement);
        setReady(true);

        cleanup = () => {
          controls.dispose();
          renderer.dispose();
          if (host?.contains(renderer.domElement)) host.removeChild(renderer.domElement);
        };
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [doc, bytes]);

  if (err) {
    return <div className="p-4 text-red-400 text-[12px]">3D 模型解析失败：{err}</div>;
  }

  return (
    <div className="flex-1 overflow-auto p-4 flex items-center justify-center bg-black/20">
      <div className="relative" style={{ width: CANVAS_W, height: CANVAS_H }} ref={mountRef}>
        {ready && activeUnitId && (
          <div className="absolute inset-0">{renderOverlay(activeUnitId, { width: CANVAS_W, height: CANVAS_H })}</div>
        )}
      </div>
    </div>
  );
}

function Cad2DView({ bytes, activeUnitId, onUnitsResolved, renderOverlay }: ViewerProps) {
  const [canvasEl, setCanvasEl] = useState<HTMLCanvasElement | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    onUnitsResolved([singleUnit("CAD 图纸", { width: CANVAS_W, height: CANVAS_H })]);
  }, [onUnitsResolved]);

  useEffect(() => {
    let cancelled = false;
    if (!canvasEl) return;
    setReady(false);
    (async () => {
      try {
        const { default: DxfParser } = await import("dxf-parser");
        const text = new TextDecoder().decode(bytes);
        const dxf = new DxfParser().parseSync(text);
        if (cancelled || !dxf) return;
        drawDxf(canvasEl, dxf);
        setReady(true);
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [bytes, canvasEl]);

  if (err) {
    return <div className="p-4 text-red-400 text-[12px]">DXF 解析失败：{err}</div>;
  }

  return (
    <div className="flex-1 overflow-auto p-4 flex items-center justify-center bg-black/20">
      <div className="relative" style={{ width: CANVAS_W, height: CANVAS_H }}>
        <canvas ref={setCanvasEl} width={CANVAS_W} height={CANVAS_H} className="bg-white block" />
        {ready && activeUnitId && (
          <div className="absolute inset-0">{renderOverlay(activeUnitId, { width: CANVAS_W, height: CANVAS_H })}</div>
        )}
      </div>
    </div>
  );
}

/**
 * 只画常见实体（LINE / LWPOLYLINE / POLYLINE / CIRCLE / ARC），够看出图纸轮廓即可，
 * 不追求完整 CAD 渲染保真度（TEXT/SPLINE/HATCH 等 v1 跳过）。
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function drawDxf(canvas: HTMLCanvasElement, dxf: any) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const entities: any[] = dxf.entities ?? [];

  // 先算所有点的包围盒，再算缩放把图纸整体塞进画布（留 5% 边距）
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  const points: { x: number; y: number }[] = [];
  for (const e of entities) {
    if (e.type === "LINE" && e.vertices?.length >= 2) points.push(...e.vertices);
    if ((e.type === "LWPOLYLINE" || e.type === "POLYLINE") && e.vertices) points.push(...e.vertices);
    if (e.type === "CIRCLE" || e.type === "ARC") {
      const r = e.radius ?? 0;
      points.push({ x: e.center.x - r, y: e.center.y - r }, { x: e.center.x + r, y: e.center.y + r });
    }
  }
  for (const p of points) {
    minX = Math.min(minX, p.x);
    minY = Math.min(minY, p.y);
    maxX = Math.max(maxX, p.x);
    maxY = Math.max(maxY, p.y);
  }
  const w = maxX - minX || 1;
  const h = maxY - minY || 1;
  const pad = 0.05;
  const scale = Math.min((canvas.width * (1 - 2 * pad)) / w, (canvas.height * (1 - 2 * pad)) / h);
  const ox = canvas.width / 2 - ((minX + maxX) / 2) * scale;
  const oy = canvas.height / 2 + ((minY + maxY) / 2) * scale; // DXF y 轴朝上，canvas 朝下，取负号翻转

  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "#1a1a1a";
  ctx.lineWidth = 1;

  const tx = (x: number) => ox + x * scale;
  const ty = (y: number) => oy - y * scale;

  for (const e of entities) {
    ctx.beginPath();
    if (e.type === "LINE" && e.vertices?.length >= 2) {
      ctx.moveTo(tx(e.vertices[0].x), ty(e.vertices[0].y));
      ctx.lineTo(tx(e.vertices[1].x), ty(e.vertices[1].y));
    } else if ((e.type === "LWPOLYLINE" || e.type === "POLYLINE") && e.vertices?.length) {
      ctx.moveTo(tx(e.vertices[0].x), ty(e.vertices[0].y));
      for (const v of e.vertices.slice(1)) ctx.lineTo(tx(v.x), ty(v.y));
      if (e.shape) ctx.closePath();
    } else if (e.type === "CIRCLE") {
      ctx.arc(tx(e.center.x), ty(e.center.y), e.radius * scale, 0, Math.PI * 2);
    } else if (e.type === "ARC") {
      ctx.arc(tx(e.center.x), ty(e.center.y), e.radius * scale, -e.endAngle, -e.startAngle);
    } else {
      continue;
    }
    ctx.stroke();
  }
}
