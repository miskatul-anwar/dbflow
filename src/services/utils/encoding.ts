/**
 * Character-encoding detection for CSV input (BOM, UTF-16 without BOM, strict UTF-8, Windows-1252).
 * Implemented in Rust (crates/dbflow-core/src/encoding.rs).
 */
import { decodeBytesRust, detectEncodingRust } from '../../engine';

export type DetectedEncoding = 'utf-8' | 'utf-16le' | 'utf-16be' | 'windows-1252';

export function detectEncoding(bytes: Uint8Array): DetectedEncoding {
  return detectEncodingRust(bytes) as DetectedEncoding;
}

/** A decoder for a file, chosen from its first bytes. */
export function createDecoder(firstBytes: Uint8Array): { decoder: TextDecoder; encoding: DetectedEncoding } {
  const encoding = detectEncoding(firstBytes);
  return { decoder: new TextDecoder(encoding), encoding };
}

export function decodeBytes(bytes: Uint8Array): { text: string; encoding: DetectedEncoding } {
  const r = decodeBytesRust(bytes);
  return { text: r.text, encoding: r.encoding as DetectedEncoding };
}
