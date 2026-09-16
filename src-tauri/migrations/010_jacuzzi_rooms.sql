-- Inventario confirmado: 19 normales (01-19) y 4 con jacuzzi (20-23).
UPDATE rooms SET room_type = 'Jacuzzi' WHERE number IN ('20', '21', '22', '23');
UPDATE rooms SET room_type = 'Normal' WHERE number NOT IN ('20', '21', '22', '23');
