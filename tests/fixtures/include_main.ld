INCLUDE "include_memory.ld"

SECTIONS
{
  .text : { *(.text*) } > FLASH
  .data : { *(.data*) } > RAM
}
