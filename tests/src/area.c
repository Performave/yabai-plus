struct test_area
{
    struct area area;
    CGPoint area_max;
};

static inline struct area make_test_area(float x, float y, float w, float h)
{
    return (struct area) { x, y, w, h };
}

static inline struct test_area make_test_area_with_max(float x, float y, float w, float h)
{
    struct test_area result;
    result.area = make_test_area(x, y, w, h);
    result.area_max = area_max_point(result.area);
    return result;
}

static inline void init_test_display_list(struct test_area display_list[3])
{
    display_list[0].area.x   = 0;
    display_list[0].area.y   = 0;
    display_list[0].area.w   = 2560;
    display_list[0].area.h   = 1440;
    display_list[0].area_max = area_max_point(display_list[0].area);

    display_list[1].area.x   = -1728;
    display_list[1].area.y   = 0;
    display_list[1].area.w   = 1728;
    display_list[1].area.h   = 1117;
    display_list[1].area_max = area_max_point(display_list[1].area);

    display_list[2].area.x   = 2560;
    display_list[2].area.y   = 0;
    display_list[2].area.w   = 1920;
    display_list[2].area.h   = 1080;
    display_list[2].area_max = area_max_point(display_list[2].area);
}

TEST_FUNC(display_area_is_in_direction,
{
    struct test_area display_list[3];
    init_test_display_list(display_list);

    bool t1 = area_is_in_direction(&display_list[0].area, display_list[0].area_max, &display_list[1].area, display_list[1].area_max, DIR_WEST);
    TEST_CHECK(t1, true);

    bool t2 = area_is_in_direction(&display_list[0].area, display_list[0].area_max, &display_list[1].area, display_list[1].area_max, DIR_EAST);
    TEST_CHECK(t2, false);

    bool t3 = area_is_in_direction(&display_list[0].area, display_list[0].area_max, &display_list[2].area, display_list[2].area_max, DIR_WEST);
    TEST_CHECK(t3, false);

    bool t4 = area_is_in_direction(&display_list[0].area, display_list[0].area_max, &display_list[2].area, display_list[2].area_max, DIR_EAST);
    TEST_CHECK(t4, true);
});

TEST_FUNC(area_max_point_uses_inclusive_bounds,
{
    struct area area = make_test_area(10, 20, 50, 30);
    CGPoint max = area_max_point(area);

    TEST_CHECK((int) max.x, 59);
    TEST_CHECK((int) max.y, 49);
});

TEST_FUNC(area_is_in_vertical_direction,
{
    struct test_area source = make_test_area_with_max(0, 0, 100, 100);
    struct test_area north = make_test_area_with_max(10, -80, 50, 50);
    struct test_area south = make_test_area_with_max(10, 120, 50, 50);
    struct test_area north_east = make_test_area_with_max(120, -80, 50, 50);

    bool t1 = area_is_in_direction(&source.area, source.area_max, &north.area, north.area_max, DIR_NORTH);
    TEST_CHECK(t1, true);

    bool t2 = area_is_in_direction(&source.area, source.area_max, &north.area, north.area_max, DIR_SOUTH);
    TEST_CHECK(t2, false);

    bool t3 = area_is_in_direction(&source.area, source.area_max, &south.area, south.area_max, DIR_SOUTH);
    TEST_CHECK(t3, true);

    bool t4 = area_is_in_direction(&source.area, source.area_max, &north_east.area, north_east.area_max, DIR_NORTH);
    TEST_CHECK(t4, false);
});

TEST_FUNC(area_make_pair_splits_with_gap,
{
    struct area parent_y = make_test_area(0, 0, 101, 50);
    struct area left_y;
    struct area right_y;
    area_make_pair(SPLIT_Y, 1, 0.5f, &parent_y, &left_y, &right_y);

    TEST_CHECK((int) left_y.x, 0);
    TEST_CHECK((int) left_y.w, 50);
    TEST_CHECK((int) right_y.x, 51);
    TEST_CHECK((int) right_y.w, 50);

    struct area parent_x = make_test_area(0, 0, 50, 101);
    struct area left_x;
    struct area right_x;
    area_make_pair(SPLIT_X, 1, 0.5f, &parent_x, &left_x, &right_x);

    TEST_CHECK((int) left_x.y, 0);
    TEST_CHECK((int) left_x.h, 50);
    TEST_CHECK((int) right_x.y, 51);
    TEST_CHECK((int) right_x.h, 50);
});

static inline int closest_display_in_direction(struct test_area *display_list, int display_count, int source, int direction)
{
    int best_index    = -1;
    int best_distance = INT_MAX;

    for (int i = 0; i < display_count; ++i) {
        if (i == source) continue;

        if (area_is_in_direction(&display_list[source].area, display_list[source].area_max, &display_list[i].area, display_list[i].area_max, direction)) {
            int distance = area_distance_in_direction(&display_list[source].area, display_list[source].area_max, &display_list[i].area, display_list[i].area_max, direction);
            if (distance < best_distance) {
                best_index = i;
                best_distance = distance;
            }
        }
    }

    return best_index;
}

TEST_FUNC(closest_display_in_direction,
{
    int best_index;
    struct test_area display_list[3];
    init_test_display_list(display_list);

    best_index = closest_display_in_direction(display_list, array_count(display_list), 0, DIR_WEST);
    TEST_CHECK(best_index, 1);

    best_index = closest_display_in_direction(display_list, array_count(display_list), 1, DIR_WEST);
    TEST_CHECK(best_index, -1);

    best_index = closest_display_in_direction(display_list, array_count(display_list), 2, DIR_WEST);
    TEST_CHECK(best_index, 0);

    best_index = closest_display_in_direction(display_list, array_count(display_list), 0, DIR_EAST);
    TEST_CHECK(best_index, 2);

    best_index = closest_display_in_direction(display_list, array_count(display_list), 1, DIR_EAST);
    TEST_CHECK(best_index, 0);

    best_index = closest_display_in_direction(display_list, array_count(display_list), 2, DIR_EAST);
    TEST_CHECK(best_index, -1);
});
