const EARTH_RADIUS_KM: f64 = 6371.0;

pub fn approximate(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let lat1 = lat1.to_radians();
    let lat2 = lat2.to_radians();
    let d_lat = (lat2 - lat1).abs();
    let d_lng = (lng2.to_radians() - lng1.to_radians()).abs();
    let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    EARTH_RADIUS_KM * c
}

#[cfg(test)]
mod tests {
    use super::{approximate, haversine_km};

    #[test]
    fn coordenada_publica_fica_com_duas_casas() {
        assert_eq!(approximate(-23.561414), -23.56);
        assert_eq!(approximate(-46.655881), -46.66);
    }

    #[test]
    fn distancia_entre_dois_pontos_de_sao_paulo() {
        let km = haversine_km(-23.5614, -46.6558, -23.5874, -46.6576);
        assert!(km > 2.0 && km < 4.0, "distância inesperada: {km}");
        assert!(haversine_km(-23.56, -46.66, -23.56, -46.66) < 0.001);
    }
}
