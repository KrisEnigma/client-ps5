/* Force-included into the console's C builds (scripts/cargo-ps5.sh). The link binds the name to
 * pf-ps5-sys's pf_ps5_getentropy (scripts/link-title.sh). */
#ifndef __ASSEMBLER__
#include <stddef.h>
int getentropy(void *buf, size_t len);
#endif
