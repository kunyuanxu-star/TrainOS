#!/usr/bin/env python3
"""Generate research report from deep research JSON results."""

import json
import os
from pathlib import Path

TOPIC_DIR = Path("/home/xukunyuan/code/AI4OSE/testOS/TrainOS/os-ccfa-research-2024-2026")
RESULTS_DIR = TOPIC_DIR / "results"
FIELDS_FILE = TOPIC_DIR / "fields.yaml"
OUTPUT_FILE = TOPIC_DIR / "report.md"

# Category grouping for report sections
CATEGORIES = {
    "基本信息": ["title", "conference", "year", "authors", "institution", "status"],
    "技术方向": ["paper_theme", "key_idea", "keywords", "technical_contribution_type"],
    "TrainOS相关性": ["relevance", "applicable_techniques", "roadmap_mapping",
                     "compatibility_with_rust", "implementation_readiness"],
    "影响力与成熟度": ["maturity", "open_source", "repo_url",
                      "citations_impact", "ecosystem_relevance"],
    "行动指南": ["effort_estimate", "time_sensitivity", "engagement_level",
                "risks_and_caveats", "suggested_next_step"],
}

# Theme labels
THEME_LABELS = {
    "formal_verification": "形式化验证",
    "memory_management": "内存管理",
    "scheduling": "调度",
    "security": "安全",
    "rust_lang": "Rust语言级",
    "disaggregated_memory": "分解内存",
    "virtualization": "虚拟化",
    "kernel_extension": "内核扩展",
    "ai_os": "AI+OS协同",
    "file_storage": "文件/存储",
    "wasm": "WebAssembly",
    "microkernel_design": "微内核设计",
    "confidential_computing": "机密计算",
    "tiered_memory": "分层内存",
    "serverless": "无服务器",
}

RELEVANCE_LABELS = {"high": "高", "medium": "中", "low": "低"}

def load_json(path):
    with open(path) as f:
        return json.load(f)

def load_fields():
    """Parse fields.yaml to get field order."""
    fields = []
    with open(FIELDS_FILE) as f:
        content = f.read()
    # Simple parsing by looking for "- name:" lines
    for line in content.split("\n"):
        stripped = line.strip()
        if stripped.startswith("- name:"):
            field_name = stripped.split(":", 1)[1].strip()
            fields.append(field_name)
    return fields

def format_value(val, indent=0):
    """Format a value for markdown display."""
    prefix = "  " * indent
    if isinstance(val, list):
        if not val:
            return "_无_"
        # Check if list of dicts
        if all(isinstance(v, dict) for v in val):
            lines = []
            for item in val:
                parts = " | ".join(f"{k}: {v}" for k, v in item.items())
                lines.append(f"{prefix}- {parts}")
            return "\n".join(lines)
        # Short list: inline. Long list: bullet
        text = ", ".join(str(v) for v in val)
        if len(text) > 120:
            return "\n".join(f"{prefix}- {v}" for v in val)
        return text
    elif isinstance(val, dict):
        lines = []
        for k, v in val.items():
            lines.append(f"{prefix}- **{k}**: {format_value(v, indent+1)}")
        return "\n".join(lines)
    elif isinstance(val, str) and len(val) > 200:
        return val  # keep as-is, long text
    else:
        return str(val) if val is not None else "_无_"

def should_skip(key, value, uncertain_list):
    """Skip uncertain or empty values."""
    if key in (uncertain_list or []):
        return True
    if value is None or value == "":
        return True
    if isinstance(value, str) and "[不确定]" in value:
        return True
    return False

def generate_report():
    fields_order = load_fields()
    results = {}

    # Load all JSON results
    for fpath in sorted(RESULTS_DIR.glob("*.json")):
        data = load_json(fpath)
        item_id = fpath.stem
        results[item_id] = data

    lines = []
    lines.append("# 操作系统 CCF-A 会议论文调研报告")
    lines.append("")
    lines.append(f"**调研日期**: 2026-05-24 | **范围**: SOSP/OSDI/EuroSys/ASPLOS/USENIX ATC 2024-2026")
    lines.append(f"**调研项数**: {len(results)} | **字段数**: {len(fields_order)}")
    lines.append("")

    # ── Executive Summary ──
    lines.append("## 执行摘要")
    lines.append("")

    # Count high-relevance
    high_items = [r for r in results.values() if r.get("relevance") == "high"]
    lines.append(f"本次调研覆盖 **{len(results)}** 篇/项 CCF-A 会议论文及技术趋势，其中 **{len(high_items)}** 项对 TrainOS 具有高参考价值。")
    lines.append("")

    lines.append("### 核心发现")
    lines.append("")
    lines.append("1. **微内核产业化已验证** — HongMeng (OSDI'24) 在数亿设备上证明了微内核架构的可行性，三级隔离+灵活组合是可直接借鉴的核心设计")
    lines.append("2. **Rust OS 形式化验证取得突破** — CortenMM (SOSP'25 Best Paper) 用 Verus 证明了并发 MMU 操作的正确性，是 TrainOS 同生态的直接对标")
    lines.append("3. **WASM 作为 eBPF 替代方案** — EuroSys'25 和 CMU 2025 分别从用户态和内核态探索 WASM 在 OS 中的角色，共识趋向 eBPF(热路径)+WASM(复杂逻辑)混合架构")
    lines.append("4. **AI+OS 协同是最大新兴方向** — OSDI'24 有 8 篇 LLM 推理论文，P/D 分离架构天然适合微内核")
    lines.append("5. **机密计算+微内核是自然组合** — TEEM³ (ASPLOS'26) 基于 M³ 微内核实现异构 TEE，TrainOS 可沿此路线扩展")
    lines.append("")

    # ── Table of Contents ──
    lines.append("## 目录")
    lines.append("")
    lines.append("| # | 论文/项目 | 会议 | 主题 | 相关性 | 路线图映射 |")
    lines.append("|---|----------|------|------|--------|-----------|")

    for idx, (item_id, data) in enumerate(sorted(results.items()), 1):
        title = data.get("title", item_id)
        conf = data.get("conference", "")
        year = data.get("year", "")
        theme = THEME_LABELS.get(data.get("paper_theme", ""), data.get("paper_theme", ""))
        relevance = RELEVANCE_LABELS.get(data.get("relevance", ""), data.get("relevance", ""))
        roadmap = data.get("roadmap_mapping", "")

        # Shorten title for TOC
        short_title = title[:60] + "..." if len(title) > 60 else title
        lines.append(f"| {idx} | [{short_title}](#{item_id.lower()}) | {conf} {year} | {theme} | {relevance} | {roadmap} |")

    lines.append("")

    # ── Detailed Sections ──
    lines.append("## 详细内容")
    lines.append("")

    for item_id, data in sorted(results.items()):
        uncertain_list = data.get("uncertain", [])

        lines.append(f"### {item_id}")
        lines.append("")

        for cat_name, cat_fields in CATEGORIES.items():
            # Check if any field in this category has a non-empty, non-uncertain value
            has_content = False
            cat_lines = []
            cat_lines.append(f"**{cat_name}**")
            cat_lines.append("")

            for field in cat_fields:
                if field in data and not should_skip(field, data[field], uncertain_list):
                    has_content = True
                    val = format_value(data[field])
                    field_display = field.replace("_", " ").title()
                    cat_lines.append(f"- **{field_display}**: {val}")

            if has_content:
                lines.extend(cat_lines)
                lines.append("")

        # Additional fields not in defined categories
        defined_fields = set()
        for cat_fields in CATEGORIES.values():
            defined_fields.update(cat_fields)
        defined_fields.update(["_source_file", "uncertain"])

        extra_fields = {}
        for key, val in data.items():
            if key not in defined_fields and not should_skip(key, val, uncertain_list):
                extra_fields[key] = val

        if extra_fields:
            lines.append("**其他信息**")
            lines.append("")
            for key, val in extra_fields.items():
                lines.append(f"- **{key}**: {format_value(val)}")
            lines.append("")

        # Uncertain fields note
        if uncertain_list:
            lines.append(f"**不确定字段**: {', '.join(uncertain_list)}")
            lines.append("")

        lines.append("---")
        lines.append("")

    # ── Cross-cutting Analysis ──
    lines.append("## 交叉分析")
    lines.append("")

    # By roadmap mapping
    roadmap_groups = {}
    for item_id, data in sorted(results.items()):
        rm = data.get("roadmap_mapping", "other")
        if rm not in roadmap_groups:
            roadmap_groups[rm] = []
        roadmap_groups[rm].append((item_id, data.get("title", item_id)))

    lines.append("### 按路线图阶段")
    lines.append("")
    for rm, items in sorted(roadmap_groups.items()):
        item_list = ", ".join(f"[{iid}](#{iid.lower()})" for iid, _ in items)
        lines.append(f"- **{rm}**: {item_list}")
    lines.append("")

    # By relevance
    lines.append("### 按相关性")
    lines.append("")
    for level in ["high", "medium", "low"]:
        items = [(iid, d.get("title", iid)) for iid, d in sorted(results.items()) if d.get("relevance") == level]
        if items:
            label = RELEVANCE_LABELS.get(level, level)
            lines.append(f"- **{label}相关性 ({len(items)}项)**: {' | '.join(f'[{iid}](#{iid.lower()})' for iid, _ in items)}")
    lines.append("")

    # By theme
    lines.append("### 按技术主题")
    lines.append("")
    theme_groups = {}
    for item_id, data in sorted(results.items()):
        theme = data.get("paper_theme", "other")
        if theme not in theme_groups:
            theme_groups[theme] = []
        theme_groups[theme].append(item_id)

    for theme, items in sorted(theme_groups.items()):
        label = THEME_LABELS.get(theme, theme)
        item_list = ", ".join(f"[{iid}](#{iid.lower()})" for iid in items)
        lines.append(f"- **{label}** ({len(items)}项): {item_list}")
    lines.append("")

    # ── TrainOS Next-Phase Recommendations ──
    lines.append("## TrainOS 下一阶段建议")
    lines.append("")
    lines.append("基于本次调研，建议下一阶段演进方向：")
    lines.append("")
    lines.append("### V31 — 内存架构重构 (CortenMM 启发)")
    lines.append("- 引入单层(One-Level)内存管理，消除 Sv39 页表之上的 VMA 软件抽象")
    lines.append("- 使用 Verus 形式化验证并发页表操作的正确性")
    lines.append("- 参考 Asterinas vostd 验证框架，建立 TrainOS 形式化验证基础设施")
    lines.append("")
    lines.append("### V32 — WASM 运行时增强 (WABI + WASMEXT 启发)")
    lines.append("- 将 WASM syscall 暴露为 host function，不破坏沙箱隔离")
    lines.append("- 采用 eBPF(热路径 hook) + 用户态 WASM(复杂策略) 混合架构")
    lines.append("- 参考 WALI 的 137 syscall 兼容列表和名称绑定机制")
    lines.append("")
    lines.append("### V33 — 机密计算扩展 (TEEM³ + Confidential Computing Trend 启发)")
    lines.append("- 基于 RISC-V Keystone/PMP 实现最小 TCB TEE")
    lines.append("- 参考 TEEM³ 核心独立 TEE 设计，支持异构硬件(CPU + AI 加速器)")
    lines.append("- 微内核 TCB 天然小而可审计，是机密计算的理想基础")
    lines.append("")
    lines.append("### V34 — AI 原生调度增强 (AI-OS Trend 启发)")
    lines.append("- 实现 P/D(预填充/解码)分离架构，映射到微内核独立服务")
    lines.append("- KV-cache 分页管理，利用已有页表机制管理 GPU 缓存")
    lines.append("- GPU-CPU 异构推理调度，利用 V25 的 NUMA 感知基础设施")
    lines.append("")

    # ── Comparison Matrix ──
    lines.append("## 快速对比矩阵")
    lines.append("")
    lines.append("| 论文 | 成熟度 | Rust兼容 | 可实施性 | 时效性 | 建议行动 |")
    lines.append("|------|--------|----------|---------|--------|---------|")

    for item_id, data in sorted(results.items()):
        title = data.get("title", item_id)[:40]
        maturity = data.get("maturity", "")
        rust = data.get("compatibility_with_rust", "")
        readiness = data.get("implementation_readiness", "")
        time_sens = data.get("time_sensitivity", "")
        engagement = data.get("engagement_level", "")
        lines.append(f"| [{item_id}](#{item_id.lower()}) | {maturity} | {rust} | {readiness} | {time_sens} | {engagement} |")

    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("*报告由 research-report skill 自动生成 | 数据来源: web search + deep research agents*")

    # Write output
    with open(OUTPUT_FILE, "w") as f:
        f.write("\n".join(lines))

    print(f"Report generated: {OUTPUT_FILE}")
    print(f"Items: {len(results)}")
    print(f"Size: {len(lines)} lines")

if __name__ == "__main__":
    generate_report()
