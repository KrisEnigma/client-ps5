/* Link-only stub for libSceVideodec2: the names the client imports, no code. The console's
 * module provides the bodies; the SDK ships no stub for it. */
#define STUB(name) __asm__(".global " #name "\n.type " #name " @function\n" #name ":\n")
STUB(sceVideodec2QueryComputeMemoryInfo);
STUB(sceVideodec2AllocateComputeQueue);
STUB(sceVideodec2ReleaseComputeQueue);
STUB(sceVideodec2QueryDecoderMemoryInfo);
STUB(sceVideodec2CreateDecoder);
STUB(sceVideodec2DeleteDecoder);
STUB(sceVideodec2Reset);
STUB(sceVideodec2Decode);
STUB(sceVideodec2Flush);
