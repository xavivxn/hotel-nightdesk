-- Inventario confirmado: jacuzzi en 01-04, normales en 05-23.
UPDATE rooms SET room_type = 'Jacuzzi' WHERE number IN ('01', '02', '03', '04');
UPDATE rooms SET room_type = 'Normal' WHERE number NOT IN ('01', '02', '03', '04');
