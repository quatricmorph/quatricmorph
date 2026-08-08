#!/usr/bin/env python3
"""
Chuyển đổi file .safetensors sang JSON.

Có 2 chế độ:
1. --mode header (mặc định): chỉ xuất phần header/metadata
   (tên tensor, dtype, shape, offset) — nhanh, file JSON nhỏ.
2. --mode full: xuất luôn toàn bộ dữ liệu tensor dưới dạng list
   số thực (list). CẢNH BÁO: file JSON có thể lớn hơn rất nhiều
   so với file .safetensors gốc (JSON lưu số dạng text, không nén).

Cài thư viện cần thiết:
    pip install safetensors numpy --break-system-packages

Cách dùng:
    python safetensors_to_json.py model.safetensors
    python safetensors_to_json.py model.safetensors -o output.json
    python safetensors_to_json.py model.safetensors --mode full
"""

import argparse
import json
import struct
import sys
from pathlib import Path


def read_header(path: Path) -> dict:
    """Đọc phần header JSON ở đầu file safetensors (8 byte độ dài + JSON)."""
    with open(path, "rb") as f:
        length_bytes = f.read(8)
        if len(length_bytes) < 8:
            raise ValueError("File không hợp lệ hoặc quá ngắn.")
        header_len = struct.unpack("<Q", length_bytes)[0]
        header_json = f.read(header_len)
        header = json.loads(header_json)
    return header


def convert_header_only(input_path: Path, output_path: Path):
    header = read_header(input_path)
    # __metadata__ là key đặc biệt (nếu có), phần còn lại là thông tin tensor
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(header, f, indent=2, ensure_ascii=False)
    print(f"Đã ghi metadata/header vào: {output_path}")


def convert_full(input_path: Path, output_path: Path):
    try:
        from safetensors import safe_open
    except ImportError:
        print(
            "Thiếu thư viện 'safetensors'. Cài bằng:\n"
            "  pip install safetensors numpy --break-system-packages",
            file=sys.stderr,
        )
        sys.exit(1)

    result = {}
    with safe_open(str(input_path), framework="numpy") as f:
        for key in f.keys():
            tensor = f.get_tensor(key)
            result[key] = {
                "dtype": str(tensor.dtype),
                "shape": list(tensor.shape),
                "data": tensor.tolist(),
            }

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False)
    print(f"Đã ghi toàn bộ dữ liệu tensor vào: {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Chuyển .safetensors sang .json")
    parser.add_argument("input", type=str, help="Đường dẫn file .safetensors")
    parser.add_argument(
        "-o", "--output", type=str, default=None,
        help="Đường dẫn file .json đầu ra (mặc định: cùng tên, đổi đuôi .json)"
    )
    parser.add_argument(
        "--mode", choices=["header", "full"], default="header",
        help="'header': chỉ metadata (mặc định, nhanh, nhẹ). "
             "'full': xuất toàn bộ dữ liệu tensor (chậm, file rất lớn)."
    )
    args = parser.parse_args()

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Không tìm thấy file: {input_path}", file=sys.stderr)
        sys.exit(1)

    output_path = Path(args.output) if args.output else input_path.with_suffix(".json")

    if args.mode == "header":
        convert_header_only(input_path, output_path)
    else:
        convert_full(input_path, output_path)


if __name__ == "__main__":
    main()
