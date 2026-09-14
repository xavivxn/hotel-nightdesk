PRAGMA foreign_keys = ON;

-- Mantener el historial y retirar del tablero las habitaciones heredadas que exceden
-- el alcance confirmado. Si una habitación tiene una estadía abierta o una reserva
-- vigente, se conserva activa hasta que pueda retirarse sin interrumpir la operación.
ALTER TABLE rooms ADD COLUMN active INTEGER NOT NULL DEFAULT 1;

UPDATE rooms
SET active = 0
WHERE id IN (
    SELECT r.id
    FROM rooms r
    WHERE r.active = 1
      AND NOT EXISTS (
          SELECT 1 FROM stays s
          WHERE s.room_id = r.id AND s.status = 'open'
      )
      AND NOT EXISTS (
          SELECT 1 FROM reservations res
          WHERE res.room_id = r.id AND res.status = 'hold'
      )
    ORDER BY CAST(r.number AS INTEGER) DESC, r.id DESC
    LIMIT MAX(
        (SELECT COUNT(*) FROM rooms WHERE active = 1) - 23,
        0
    )
);
