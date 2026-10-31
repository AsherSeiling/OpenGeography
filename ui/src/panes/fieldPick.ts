// Which field a new graduated / unique-value style starts on: the first one that is not an identifier (FID, OBJECTID, CELL_ID, ROW …).
const ID_LIKE = /^(fid|ogc_fid|objectid|oid|gid|id|cell_id|row|col|rn|__fid)$|_id$/i;

export function defaultClassField<T extends { name: string }>(fields: T[]): string | undefined {
  return (fields.find((f) => !ID_LIKE.test(f.name)) ?? fields[0])?.name;
}
