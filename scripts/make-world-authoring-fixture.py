#!/usr/bin/env python3
"""生成确定性 0.15 作者验收负载；纯标准库，不执行 worldline 或启动编辑器。

已有非空输出目录一律拒绝改写。--check 只验证既有负载原字节。
使用：python3 scripts/make-world-authoring-fixture.py --output ../native-fixture
"""
import argparse
import hashlib
import io
import json
import os
import platform
import struct
import sys
import wave
import zlib
from pathlib import Path

FEATURE = "presentation.vector_scene.v1"
COUNT = 5000
PALETTE = ["#4e79a7", "#59a14f", "#f28e2b", "#e15759", "#b07aa1"]


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode()


def scene(width, height):
    return {"schema_version": 1, "view_box": [0, 0, width, height],
            "preserve_aspect_ratio": "xMidYMid meet", "root_order": {}, "nodes": {}}


def node(name, layer, geometry, **fields):
    value = {"id": name, "name": name, "layer_id": layer, "geometry": geometry,
             "transform": [1, 0, 0, 1, 0, 0], "style": {}, "visible": True,
             "locked": False, "annotation": "确定性作者验收图元", "scope_refs": []}
    value.update(fields)
    return value


def rect(x, y, width, height):
    return {"kind": "rect", "x": x, "y": y, "width": width, "height": height, "rx": 0, "ry": 0}


def map_document(name, title, width=1000, height=600):
    return {"schema_version": 1, "id": name, "title": title, "required_features": [FEATURE],
            "canvas": {"width": width, "height": height, "unit": "normalized"},
            "raster_layers": [], "layer_order": [], "layers": {}, "placements": {},
            "scene": scene(width, height), "extensions": {"fixture": "world-authoring-v015-fixed-v1"}}


def add_layer(document, name, title, visible=True):
    document["layer_order"].append(name)
    document["layers"][name] = {"title": title, "visible_default": visible, "locked": False}
    document["scene"]["root_order"][name] = []


def add_node(document, value, root=True):
    document["scene"]["nodes"][value["id"]] = value
    if root:
        document["scene"]["root_order"][value["layer_id"]].append(value["id"])


def dense_map():
    document = map_document("b_dense", "单层 5000 · CPU 帧负载")
    add_layer(document, "dense", "5000 个矩形")
    for index in range(COUNT):
        value = node(f"rect_{index:04}", "dense", rect((index % 100) * 10, (index // 100) * 12, 8, 10),
                     style={"fill": PALETTE[index % len(PALETTE)], "stroke": "#203040", "stroke_width": 0.4})
        if index == 0:
            value["target_ref"] = {"kind": "entity", "id": "harbor"}
        add_node(document, value)
    return document


def pressure_map():
    document = map_document("c_layers_pressure", "5000 层 · RGBA 容量压力（预期提示超额）")
    for index in range(COUNT):
        layer = f"layer_{index:04}"
        add_layer(document, layer, f"压力层 {index:04}")
        add_node(document, node(f"plane_{index:04}", layer, rect(0, 0, 1000, 600),
                               style={"fill": PALETTE[index % len(PALETTE)], "fill_opacity": 0.12, "stroke": "none"}))
    return document


def workflow_map():
    document = map_document("a_workflow", "作者闭环 · 雾港 / Harbor", 400, 200)
    document["required_features"] += ["presentation.geometry.line_area.v1", "presentation.geometry.text.v1", "presentation.measurement.v1"]
    add_layer(document, "places", "旧标记与可编辑矢量")
    add_layer(document, "history", "隐藏历史入口", False)
    document["raster_layers"] = [{"id": "coast", "asset": {"kind": "asset", "id": "harbor_image"}, "rect": [0, 0, 1, 1]}]
    document["measurement"] = {"points": [[0.1, 0.9], [0.9, 0.9]], "distance": 80, "unit": "海里"}
    for name, layer, target, position, navigation in [
        ("legacy_harbor", "places", {"kind": "entity", "id": "harbor"}, [0.15, 0.55], None),
        ("legacy_character", "places", {"kind": "character", "id": "lin"}, [0.3, 0.45], None),
        ("legacy_dense_link", "places", None, [0.85, 0.15], {"map_id": "b_dense"}),
        ("legacy_event", "history", {"kind": "event", "id": "beacon"}, [0.8, 0.55], None),
    ]:
        document["placements"][name] = {"layer_id": layer, "target_ref": target,
            "geometry": {"kind": "point", "position": position}, "annotation": "旧标记，可显式迁移并反查资料",
            "role": "作者验收入口", "label_override": None, "navigation": navigation, "scope_refs": []}
    document["placements"]["legacy_route"] = {"layer_id": "places", "target_ref": None, "geometry": {"kind": "polyline", "points": [[0.1, 0.75], [0.45, 0.7], [0.75, 0.4]]},
        "annotation": "旧折线、节点选择与迁移顺序", "role": "路线", "scope_refs": []}
    document["placements"]["legacy_label"] = {"layer_id": "places", "target_ref": None, "geometry": {"kind": "text", "position": [0.06, 0.08], "text": "旧标签 · Harbor", "font_size": 16, "color": "#19384a"},
        "annotation": "保留旧文字编辑能力", "role": "说明", "scope_refs": []}
    group = node("harbor_group", "places", {"kind": "group", "children": ["curve", "ellipse", "caption"]},
                 transform=[1, 0.08, 0.12, 1, 16, 10], style={"opacity": 0.8})
    add_node(document, group)
    segments = [{"kind": "move", "to": [20, 35]},
                {"kind": "cubic", "control1": [65, 5], "control2": [115, 90], "to": [155, 40]},
                {"kind": "quadratic", "control": [190, 30], "to": [185, 100]},
                {"kind": "line", "to": [25, 95]}, {"kind": "close"},
                {"kind": "move", "to": [60, 52]}, {"kind": "line", "to": [85, 52]},
                {"kind": "line", "to": [75, 76]}, {"kind": "close"}]
    add_node(document, node("curve", "places", {"kind": "path", "segments": segments}, parent_id="harbor_group",
        style={"fill": "#419b84", "stroke": "#184b50", "stroke_width": 2, "fill_rule": "evenodd"},
        target_ref={"kind": "entity", "id": "harbor"}), False)
    add_node(document, node("ellipse", "places", {"kind": "ellipse", "cx": 245, "cy": 65, "rx": 45, "ry": 22},
        parent_id="harbor_group", style={"fill": "#efb45b", "stroke": "#19384a", "stroke_width": 2}), False)
    add_node(document, node("caption", "places", {"kind": "text", "x": 185, "y": 120,
        "runs": [{"text": "雾港 Harbor", "dx": 0, "dy": 0, "style": {}},
                 {"text": " 400%", "dx": 2, "dy": 0, "style": {"font_weight": "bold"}}]},
        parent_id="harbor_group", style={"fill": "#152f49", "font_size": 15, "font_family": "Noto Sans SC"}), False)
    add_node(document, node("viewport_root", "places", {"kind": "group", "children": ["cropped_plane"]},
        transform=[1, 0, 0, 1, 292, 133], clip_rect=[0, 0, 85, 46], svg_root=True))
    add_node(document, node("cropped_plane", "places", rect(-25, -20, 140, 90), parent_id="viewport_root",
        style={"fill": "#b778aa", "stroke": "#512747", "stroke_width": 3}), False)
    add_node(document, node("hidden_event", "history", {"kind": "point", "position": [340, 140]},
        target_ref={"kind": "event", "id": "beacon"}, style={"fill": "#e15759"}))
    return document


def png_bytes():
    width, height = 400, 200
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            land = x + 2 * y < 380
            grid = (x % 25 == 0 or y % 25 == 0)
            color = ((218, 219, 192) if land else (152, 195, 210)) if not grid else (129, 159, 169)
            rows.extend((*color, 255))
    data = bytearray(b"\x78\x01")
    for offset in range(0, len(rows), 65535):
        block = rows[offset:offset + 65535]
        data.extend(bytes([int(offset + len(block) == len(rows))]))
        data.extend(struct.pack("<HH", len(block), 65535 - len(block)))
        data.extend(block)
    data.extend(struct.pack(">I", zlib.adler32(rows) & 0xffffffff))
    def chunk(name, value):
        return struct.pack(">I", len(value)) + name + value + struct.pack(">I", zlib.crc32(name + value) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)) + chunk(b"IDAT", bytes(data)) + chunk(b"IEND", b"")


def wav_bytes():
    stream = io.BytesIO()
    with wave.open(stream, "wb") as audio:
        audio.setnchannels(1); audio.setsampwidth(2); audio.setframerate(11025)
        samples = bytearray()
        for index in range(11025):
            phase = (index * 440) % 11025
            amplitude = ((phase if phase < 5513 else 11025 - phase) * 32768 // 11025) - 8192
            samples.extend(struct.pack("<h", amplitude))
        audio.writeframes(samples)
    return stream.getvalue()


def svg_workflow():
    return '''<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200" viewBox="10 20 400 200" preserveAspectRatio="xMidYMid meet">
<g transform="translate(16 12) matrix(1 0.08 0.12 1 0 0)" opacity="0.72">
<path d="M 25 50 C 65 10 110 110 160 55 S 205 45 190 115 L 30 115 Z M 60 65 Q 75 48 100 70 T 80 95 Z" fill="#419b84" fill-rule="evenodd" stroke="#184b50" stroke-width="2"/>
<rect x="220" y="45" width="80" height="45" rx="8" fill="#efb45b"/>
<ellipse cx="315" cy="120" rx="48" ry="24" fill="#b778aa"/>
<text x="35" y="165" font-size="18" fill="#152f49">雾港 Harbor<tspan dx="6" font-weight="bold"> · 曲线</tspan></text>
</g></svg>\n'''


def artifacts():
    files = {}
    def put(path, content):
        files[path] = content.encode() if isinstance(content, str) else content
    put("authoring/world.wl", '''world fog_harbor as "雾港作者验收"
  description "地图、资料与书稿共享稳定对象引用。"
  property era = "潮汐纪元"
period evening as "港口傍晚"
include "lore.wl"
include "events/arrival.wl"
asset harbor_image image "assets/harbor.png" as "雾港栅格底图"
asset harbor_audio audio "assets/harbor.wav" as "440 Hz 校验音"
''')
    put("authoring/lore.wl", '''character lin as "林舟"
  property appearance = "带着旧罗盘的守灯人。"
  property motivation = "让港口与灯塔重新取得联系。"
alias character lin as "阿舟"
entity harbor kind place as "雾港码头"
  description "潮水沿石阶退去，地图上的入口指向此处。"
entity lighthouse kind place as "孤岛灯塔"
  description "远处的灯光是另一段旅程的入口。"
relation_type connects as "连接"
  inverse "连接自"
  direction directed
relation_def harbor_route type connects from entity harbor to entity lighthouse
  description "海岸线上的作者定义路线。"
''')
    put("authoring/events/arrival.wl", '''event arrival as "抵达雾港" with lin at 1 during evening
  [[character:lin|林舟]]在[[entity:harbor|雾港码头]]展开地图。
  choice "前往灯塔"
    -> beacon
  choice "先留在码头"
    -> END

event beacon as "灯塔回应" with lin at 2 during evening follows arrival
  [[entity:lighthouse|孤岛灯塔]]照亮了海岸，Harbor lights answer the tide.
  -> END
''')
    maps = [workflow_map(), dense_map(), pressure_map()]
    for document in maps:
        put(f"authoring/.world/maps/{document['id']}.json", json_bytes(document))
    project = {"schema_version": 1, "project_id": "world_authoring_v015_fixture", "language_version": "1.10", "entry": "world.wl",
        "required_features": ["presentation.maps.v1", "presentation.manuscripts.v1", "content.entities.v1", "content.relations.v1", "content.object_refs.v1"],
        "maps": {item["id"]: f".world/maps/{item['id']}.json" for item in maps}, "graph_views": {},
        "manuscripts": {"harbor_book": ".world/manuscripts/harbor_book.json"}, "extensions": {}}
    put("authoring/.world/project.json", json_bytes(project))
    put("authoring/.world/manuscripts/harbor_book.json", json_bytes({"schema_version": 1, "id": "harbor_book", "title": "雾港 · 作者验收书稿", "entries": [
        {"id": "part_one", "kind": "section", "title": "海岸", "parent_id": None},
        {"id": "arrival_chapter", "kind": "chapter", "parent_id": "part_one", "title": "抵达", "target_ref": {"kind": "event", "id": "arrival"}, "pov": {"kind": "character", "id": "lin"}, "status": "draft"},
        {"id": "beacon_chapter", "kind": "chapter", "parent_id": "part_one", "title": "回应", "target_ref": {"kind": "event", "id": "beacon"}, "status": "draft"},
        {"id": "harbor_lore", "kind": "chapter", "title": "码头资料", "target_ref": {"kind": "entity", "id": "harbor"}, "status": "draft"}]}))
    put("authoring/assets/harbor.png", png_bytes()); put("authoring/assets/harbor.wav", wav_bytes())
    put("authoring/imports/curve-groups-text-400x200.svg", svg_workflow())
    shapes = [f'<rect x="{(i % 50) * 8}" y="{(i // 50) * 10}" width="7" height="8" fill="{PALETTE[i % 5]}"/>' for i in range(1000)]
    put("authoring/imports/shapes-1000.svg", '<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200" viewBox="0 0 400 200">\n' + "\n".join(shapes) + '\n</svg>\n')
    put("authoring/imports/root-slice.svg", '<svg xmlns="http://www.w3.org/2000/svg" width="300" height="100" viewBox="0 0 100 100" preserveAspectRatio="xMidYMid slice"><rect width="100" height="100" fill="#efb45b"/><circle cx="50" cy="50" r="25" fill="#4e79a7"/></svg>\n')
    put("default-1.9/world.wl", 'world blank_world as "默认 1.9 用例"\n  description "没有清单时继续默认 1.9，不隐式升级。"\nevent start\n  新世界尚待书写。\n  -> END\n')
    put("README.md", '''# 0.15 确定性作者验收负载 v1

主工程：authoring/world.wl，显式语言 1.10，可测新建地点与绑定；default-1.9/world.wl 没有清单，检验默认 1.9；new-empty-default/ 是真正空目录，供 UI 新建默认工程。三者是独立工作区，不混扫源码。

a_workflow：400×200，真实 PNG 底图、旧点/线/文字、隐藏事件入口、校准、原生 C/Q 多子路径/孔洞/组 opacity/Affine、中英文字和 typed viewport clip。b_dense：单层恰好5000矩形，100列×50行，1000×600逻辑画布。c_layers_pressure：5000层各1个全视口半透明矩形，预期触发明确资源容量提示；不要求超预算时全部显示，不得静默漏层或降DPI。

imports/curve-groups-text-400x200.svg 测编辑交换；shapes-1000.svg 恰好1000简单形状；root-slice.svg 测根viewport裁剪。asset harbor_image 与 harbor_audio 注册真实PNG及1秒PCM WAV（11025Hz单声道16bit，约440Hz三角校验音）。人物、地点、关系、事件、三个书稿章节都可互相定位；点击入口不执行事件。

CPU探针：WORLDEDIT_DENSE_FIXTURE=/absolute/native-fixture/authoring/world.wl cargo test --release --locked dense_scene_headless_release_profile -- --ignored --nocapture。headless不代表原生FPS/GPU。记录当前候选SHA、fixture-manifest.json、machine.json、DPI/窗口、冷热及每轮原始数据。5000节点p95≤33ms须在release固定负载下测；资源压力另看任务数、合计预算、取消/隐藏恢复。

生成器拒绝覆盖非空目录。验证原字节用 --check；作者在QA中保存修改后预期校验不再匹配，应保留修改证据并另生成新目录，不覆盖稿件。新建空目录不进入文件hash集合。
''')
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true", help="只读核验确定性文件，不覆盖QA稿件")
    args = parser.parse_args()
    files = artifacts()
    manifest = {"fixture": "world-authoring-v015-fixed-v1", "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "workloads": {"b_dense": {"scene_nodes": 5000, "layers": 1}, "c_layers_pressure": {"scene_nodes": 5000, "layers": 5000}, "svg_shapes": 1000},
        "files": [{"path": path, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()} for path, data in sorted(files.items())],
        "empty_directories": ["new-empty-default"]}
    files["fixture-manifest.json"] = json_bytes(manifest)
    output = args.output.resolve()
    if args.check:
        errors = [path for path, data in sorted(files.items()) if not (output / path).is_file() or (output / path).read_bytes() != data]
        if not (output / "new-empty-default").is_dir() or any((output / "new-empty-default").iterdir()):
            errors.append("new-empty-default must be empty")
        print(json.dumps({"checked_files": len(files), "mismatches": errors}, ensure_ascii=False))
        return bool(errors)
    if output.exists() and any(output.iterdir()):
        parser.error("输出目录非空；拒绝覆盖。请用 --check 或选择全新输出目录")
    output.mkdir(parents=True, exist_ok=True)
    for path, data in sorted(files.items()):
        target = output / path; target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as file:
            file.write(data)
    (output / "new-empty-default").mkdir(exist_ok=True)
    machine = {"system": platform.system(), "release": platform.release(), "architecture": platform.machine(),
        "python": platform.python_version(), "cpu_count": os.cpu_count(), "note": "机器描述不是性能结果；原生DPI/CPU型号/窗口与release候选SHA由验收另记。"}
    cgroup = Path("/sys/fs/cgroup/memory.max")
    if cgroup.is_file():
        machine["cgroup_memory_max"] = cgroup.read_text().strip()
    (output / "machine.json").write_bytes(json_bytes(machine))
    print(json.dumps({"output": str(output), "files": len(files), "raw_bytes": sum(map(len, files.values())),
        "manifest_sha256": hashlib.sha256(files["fixture-manifest.json"]).hexdigest()}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
