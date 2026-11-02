/** Минимальный GLB: пирамида 0,8 × 0,5 м, высота 1,2 м (ось Y — вверх) — для импорта своей модели. */
export function pyramidGlb(): Buffer {
  const positions = [0, 0, 0, 0.8, 0, 0, 0.8, 0, 0.5, 0, 0, 0.5, 0.4, 1.2, 0.25];
  const indices = [0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4, 0, 2, 1, 0, 3, 2];
  const bin = Buffer.alloc(positions.length * 4 + indices.length * 4);
  for (const [i, v] of positions.entries()) bin.writeFloatLE(v, i * 4);
  for (const [i, v] of indices.entries()) bin.writeUInt32LE(v, positions.length * 4 + i * 4);
  const json = JSON.stringify({
    asset: { version: "2.0" },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1 }] }],
    buffers: [{ byteLength: bin.length }],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: positions.length * 4 },
      { buffer: 0, byteOffset: positions.length * 4, byteLength: indices.length * 4 },
    ],
    accessors: [
      {
        bufferView: 0,
        componentType: 5126,
        count: positions.length / 3,
        type: "VEC3",
        min: [0, 0, 0],
        max: [0.8, 1.2, 0.5],
      },
      { bufferView: 1, componentType: 5125, count: indices.length, type: "SCALAR" },
    ],
  });
  let head = Buffer.from(json);
  head = Buffer.concat([head, Buffer.alloc((4 - (head.length % 4)) % 4, 0x20)]);
  const chunk = (data: Buffer, type: number) => {
    const header = Buffer.alloc(8);
    header.writeUInt32LE(data.length, 0);
    header.writeUInt32LE(type, 4);
    return Buffer.concat([header, data]);
  };
  const body = Buffer.concat([chunk(head, 0x4e4f534a), chunk(bin, 0x004e4942)]);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + body.length, 8);
  return Buffer.concat([header, body]);
}
