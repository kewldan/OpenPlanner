/** Минимальный писатель GLB (одна сетка) — для тестовых сканов. Координаты в метрах, ось Y вверх. */
export function encodeGlb(positions: number[][], triangles: number[][]): Buffer {
  const vertices = Buffer.alloc(positions.length * 12);
  positions.forEach((p, i) => {
    p.forEach((v, k) => {
      vertices.writeFloatLE(v, i * 12 + k * 4);
    });
  });
  const indices = Buffer.alloc(triangles.length * 12);
  triangles.forEach((t, i) => {
    t.forEach((v, k) => {
      indices.writeUInt32LE(v, i * 12 + k * 4);
    });
  });
  const bin = Buffer.concat([vertices, indices]);
  const json = Buffer.from(
    JSON.stringify({
      asset: { version: "2.0" },
      scene: 0,
      scenes: [{ nodes: [0] }],
      nodes: [{ mesh: 0 }],
      meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1 }] }],
      buffers: [{ byteLength: bin.length }],
      bufferViews: [
        { buffer: 0, byteOffset: 0, byteLength: vertices.length },
        { buffer: 0, byteOffset: vertices.length, byteLength: indices.length },
      ],
      accessors: [
        { bufferView: 0, componentType: 5126, count: positions.length, type: "VEC3" },
        { bufferView: 1, componentType: 5125, count: triangles.length * 3, type: "SCALAR" },
      ],
    }),
  );
  const pad = (b: Buffer, fill: number) =>
    Buffer.concat([b, Buffer.alloc((4 - (b.length % 4)) % 4, fill)]);
  const jsonChunk = pad(json, 0x20);
  const binChunk = pad(bin, 0);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + 8 + jsonChunk.length + 8 + binChunk.length, 8);
  const chunk = (data: Buffer, type: number) => {
    const head = Buffer.alloc(8);
    head.writeUInt32LE(data.length, 0);
    head.writeUInt32LE(type, 4);
    return Buffer.concat([head, data]);
  };
  return Buffer.concat([header, chunk(jsonChunk, 0x4e4f534a), chunk(binChunk, 0x004e4942)]);
}

/** Только пол 4 × 4 м сеткой 0,2 м: стен нет — распознавание должно сообщить об этом. */
export function floorOnly(): Buffer {
  const positions: number[][] = [];
  const triangles: number[][] = [];
  const n = 20;
  for (let i = 0; i <= n; i++) {
    for (let j = 0; j <= n; j++) positions.push([i * 0.2, 0, j * 0.2]);
  }
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      const a = i * (n + 1) + j;
      triangles.push([a, a + 1, a + n + 2], [a, a + n + 2, a + n + 1]);
    }
  }
  return encodeGlb(positions, triangles);
}
