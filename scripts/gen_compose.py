#!/usr/bin/env python3
import argparse
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate Docker Compose for a DL-DSR chain.")
    parser.add_argument("--nodes", type=int, default=4, choices=(4, 8, 16, 32))
    parser.add_argument("--mode", default="dldsr", choices=("dsr", "dldsr"))
    parser.add_argument("--output", default="docker-compose.generated.yml")
    args = parser.parse_args()

    lines = [
        "services:",
    ]
    for node_id in range(args.nodes):
        ip = f"172.28.0.{10 + node_id}"
        lines.extend(
            [
                f"  node{node_id}:",
                "    build:",
                "      context: .",
                "      dockerfile: docker/Dockerfile",
                "    environment:",
                f"      NODE_ID: \"{node_id}\"",
                "      BIND_ADDR: \"0.0.0.0:7000\"",
                f"      MODE: \"{args.mode}\"",
                f"      TOPOLOGY: \"/app/docker/topology/chain-{args.nodes}.toml\"",
            ]
        )
        if node_id == 0:
            lines.extend(
                [
                    f"      SEND_DST: \"{args.nodes - 1}\"",
                    "      PAYLOAD: \"hello from docker compose\"",
                    "      SEND_AFTER_MS: \"2500\"",
                ]
            )
        lines.extend(
            [
                "    networks:",
                "      dldsr-net:",
                f"        ipv4_address: {ip}",
            ]
        )

    lines.extend(
        [
            "networks:",
            "  dldsr-net:",
            "    driver: bridge",
            "    ipam:",
            "      config:",
            "        - subnet: 172.28.0.0/16",
        ]
    )

    Path(args.output).write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
