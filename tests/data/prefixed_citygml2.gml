<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<core:CityModel xmlns:core="http://www.opengis.net/citygml/2.0" xmlns:gml="http://www.opengis.net/gml" xmlns:bldg="http://www.opengis.net/citygml/building/2.0" xmlns:gen="http://www.opengis.net/citygml/generics/2.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xlink="http://www.w3.org/1999/xlink">
  <gml:boundedBy>
    <gml:Envelope srsName="http://www.opengis.net/def/crs/EPSG/0/7415" srsDimension="3">
      <gml:lowerCorner>84501.554 445805.031 -2.462</gml:lowerCorner>
      <gml:upperCorner>85675.234 446983.469 94.801</gml:upperCorner>
    </gml:Envelope>
  </gml:boundedBy>
  <core:cityObjectMember>
    <bldg:Building gml:id="building-001">
      <gml:boundedBy>
        <gml:Envelope srsDimension="3">
          <gml:lowerCorner>84537.914 445805.031 -0.41</gml:lowerCorner>
          <gml:upperCorner>84864.828 446100.844 18.823</gml:upperCorner>
        </gml:Envelope>
      </gml:boundedBy>
      <gen:stringAttribute name="b3_dak_type">
        <gen:value>slanted</gen:value>
      </gen:stringAttribute>
      <bldg:lod1Solid>
        <gml:Solid>
          <gml:exterior>
            <gml:CompositeSurface>
              <gml:surfaceMember>
                <gml:Polygon>
                  <gml:exterior>
                    <gml:LinearRing>
                      <gml:posList>84537.914 445805.031 0.0 84864.828 445805.031 0.0 84864.828 446100.844 0.0 84537.914 446100.844 0.0 84537.914 445805.031 0.0</gml:posList>
                    </gml:LinearRing>
                  </gml:exterior>
                </gml:Polygon>
              </gml:surfaceMember>
            </gml:CompositeSurface>
          </gml:exterior>
        </gml:Solid>
      </bldg:lod1Solid>
      <bldg:consistsOfBuildingPart>
        <bldg:BuildingPart gml:id="building-001-part-0">
          <bldg:lod2MultiSurface>
            <gml:MultiSurface>
              <gml:surfaceMember>
                <gml:Polygon>
                  <gml:exterior>
                    <gml:LinearRing>
                      <gml:posList>84537.914 445805.031 0.0 84700.0 445805.031 0.0 84700.0 446100.844 0.0 84537.914 446100.844 0.0 84537.914 445805.031 0.0</gml:posList>
                    </gml:LinearRing>
                  </gml:exterior>
                </gml:Polygon>
              </gml:surfaceMember>
            </gml:MultiSurface>
          </bldg:lod2MultiSurface>
        </bldg:BuildingPart>
      </bldg:consistsOfBuildingPart>
    </bldg:Building>
  </core:cityObjectMember>
  <core:cityObjectMember>
    <bldg:Building gml:id="building-002">
      <gml:boundedBy>
        <gml:Envelope srsDimension="3">
          <gml:lowerCorner>84900.0 446000.0 0.0</gml:lowerCorner>
          <gml:upperCorner>84950.0 446050.0 12.0</gml:upperCorner>
        </gml:Envelope>
      </gml:boundedBy>
      <gen:stringAttribute name="b3_dak_type">
        <gen:value>flat</gen:value>
      </gen:stringAttribute>
      <bldg:lod1Solid>
        <gml:Solid>
          <gml:exterior>
            <gml:CompositeSurface>
              <gml:surfaceMember>
                <gml:Polygon>
                  <gml:exterior>
                    <gml:LinearRing>
                      <gml:posList>84900.0 446000.0 0.0 84950.0 446000.0 0.0 84950.0 446050.0 0.0 84900.0 446050.0 0.0 84900.0 446000.0 0.0</gml:posList>
                    </gml:LinearRing>
                  </gml:exterior>
                </gml:Polygon>
              </gml:surfaceMember>
            </gml:CompositeSurface>
          </gml:exterior>
        </gml:Solid>
      </bldg:lod1Solid>
    </bldg:Building>
  </core:cityObjectMember>
  <core:cityObjectMember>
    <bldg:Building gml:id="building-003">
      <gen:stringAttribute name="b3_dak_type">
        <gen:value>flat</gen:value>
      </gen:stringAttribute>
      <bldg:lod2MultiSurface>
        <gml:MultiSurface>
          <gml:surfaceMember>
            <gml:Polygon>
              <gml:exterior>
                <gml:LinearRing>
                  <gml:posList>85000.0 446200.0 0.0 85050.0 446200.0 0.0 85050.0 446250.0 0.0 85000.0 446250.0 0.0 85000.0 446200.0 0.0</gml:posList>
                </gml:LinearRing>
              </gml:exterior>
            </gml:Polygon>
          </gml:surfaceMember>
        </gml:MultiSurface>
      </bldg:lod2MultiSurface>
    </bldg:Building>
  </core:cityObjectMember>
</core:CityModel>
