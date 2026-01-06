#!/usr/bin/env python3
"""
Export NLLB model to ONNX format for use with the embedded Rust provider.

This script uses HuggingFace's Optimum library to convert the NLLB
encoder-decoder model to ONNX format for inference with ONNX Runtime.

Usage:
    pip install optimum[exporters] transformers torch
    python scripts/export_nllb_onnx.py

Output Structure:
    models/nllb-onnx/
    +-- encoder_model.onnx          # Encodes input text to hidden states
    +-- decoder_model.onnx          # Generates output tokens (autoregressive)
    +-- decoder_model_merged.onnx   # Decoder with KV cache for efficiency
    +-- tokenizer.json              # Vocabulary and tokenization rules
    +-- config.json                 # Model configuration

Model Options (by size):
    - facebook/nllb-200-distilled-600M  (1.2GB)  - Good balance (default)
    - facebook/nllb-200-distilled-1.3B  (2.6GB)  - Better quality
    - facebook/nllb-200-3.3B            (6.6GB)  - Best quality, slow

Note on Decoder Implementation:
    The ONNX models exported here work with the Rust provider, but the
    autoregressive decoder loop is not yet implemented in Rust. The encoder
    runs successfully; completing the decoder requires managing KV-cache
    tensors across generation steps.

    For production use, consider the nllb_rest provider which uses a
    Python server with full HuggingFace transformers support.
"""

import subprocess
import sys
from pathlib import Path


def main():
    # Configuration
    # The 600M distilled model offers good quality at reasonable size
    # For better translations, use "facebook/nllb-200-distilled-1.3B"
    model_name = "facebook/nllb-200-distilled-600M"
    output_dir = Path("models/nllb-onnx")

    print("=" * 60)
    print("NLLB ONNX Export")
    print("=" * 60)
    print()
    print(f"Model: {model_name}")
    print(f"Output: {output_dir}")
    print()

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Build the export command
    # Using optimum.exporters.onnx which handles encoder-decoder models
    cmd = [
        sys.executable,
        "-m",
        "optimum.exporters.onnx",
        "--model",
        model_name,
        "--task",
        "translation",  # This tells optimum it's an encoder-decoder model
        str(output_dir),
    ]

    print("Running export command:")
    print(f"  {' '.join(cmd)}")
    print()
    print("This may take several minutes (downloading model, converting)...")
    print()

    try:
        subprocess.run(cmd, check=True)

        print()
        print("=" * 60)
        print("Export successful!")
        print("=" * 60)
        print()
        print("Files created:")

        for f in sorted(output_dir.iterdir()):
            size_mb = f.stat().st_size / (1024 * 1024)
            print(f"  {f.name:30} {size_mb:>8.1f} MB")

        print()
        print("Next steps:")
        print()
        print("  1. The Rust ONNX provider can load these models:")
        print()
        print(f"     cargo run --features nllb-onnx -- \\")
        print(f"         --provider nllb-onnx \\")
        print(f"         --model-dir {output_dir} \\")
        print(f"         --input input.csv --output output.csv")
        print()
        print("  2. Note: Decoder loop is not yet implemented in Rust.")
        print("     For production, use the REST provider instead:")
        print()
        print("     cargo run -- --provider nllb --nllb-base-url http://localhost:8080 ...")
        print()
        print("=" * 60)

    except subprocess.CalledProcessError as e:
        print(f"Error: Export failed with code {e.returncode}")
        print()
        print("Troubleshooting:")
        print("  1. Install required packages:")
        print("     pip install optimum[exporters] transformers torch")
        print()
        print("  2. Ensure you have enough disk space (~2GB for 600M model)")
        print()
        print("  3. Check your internet connection (model download required)")
        sys.exit(1)

    except FileNotFoundError:
        print("Error: optimum package not found")
        print()
        print("Install with:")
        print("  pip install optimum[exporters] transformers torch")
        sys.exit(1)


if __name__ == "__main__":
    main()
