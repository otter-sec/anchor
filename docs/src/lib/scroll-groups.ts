export function scrollGroupSectionId(docId: string): string {
  return `doc-${docId.replace(/\//g, '-') || 'index'}`
}
