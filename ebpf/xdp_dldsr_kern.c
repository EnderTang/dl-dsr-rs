// Prototype only. Build manually with clang -target bpf.
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("xdp")
int xdp_dldsr(struct xdp_md *ctx)
{
    void *data_end = (void *)(long)ctx->data_end;
    void *data = (void *)(long)ctx->data;

    if (data + 1 > data_end) {
        return XDP_PASS;
    }

    unsigned char first = *(unsigned char *)data;
    if (first == 0xff) {
        return XDP_ABORTED;
    }

    return XDP_PASS;
}

char _license[] SEC("license") = "GPL";

